//! Disk and RAM the agents leave behind (docs/perf/FIXES-2026-10-18.md).
//!
//! A session's checkout outlives it whenever the close keeps the worktree (the shell's "Discard
//! session", a dirty checkout, `remove_worktree:false`), and nothing took its build output with
//! it: ten kept checkouts held 810 GB of `target/`. Nor did anything take the provider's scratch
//! directory, `<tmp>/claude-<uid>/<checkout slug>/<session uuid>/`, where agents build and
//! profile; `/tmp` is a tmpfs, so the 12 GB they left there sat in RAM and then in swap.
//!
//! Cargo never deletes an artifact either. A selection of packages (`cargo test -p a -p b`)
//! resolves its dependencies' features apart from the whole workspace, so each new selection links
//! a whole new set of test executables beside the old ones: one live checkout wrote 202 of them,
//! 76 GB, in under an hour.
//!
//! The sweep runs on the retention timer (`purge::spawn_timer`), a minute after a session closes,
//! and on `app.reconcile`:
//! - a checkout in an agent-made area (`.claude/worktrees/`, Relay's own areas) whose sessions
//!   are all closed loses the build output git ignores ([`worktree::ignored_build_dirs`]); so
//!   does one there that no session owns, once nothing was built in it for [`UNOWNED_IDLE`]. A
//!   checkout the user made elsewhere keeps its builds even after a session worked in it, as
//!   `session.close` keeps the checkout itself. The checkout, its changes and its branch stay:
//!   those are `session.close`'s and branch cleanup's to decide.
//! - a checkout a live session owns, and the primary, keep their builds, but only the [`KEEP`]
//!   newest test executables of each name ([`trim`]).
//! - a closed session's scratch directory goes.
//!
//! Nothing is touched while someone works there: no purge while any process's cwd is inside the
//! checkout, no trim while cargo or rustc runs in it or a test executable of it is running — a
//! test run executes the binaries it judged fresh, and the trim must not take one from under it.
//! A first pass over large builds takes minutes, so the process table is read again for every
//! checkout, and the store is asked again just before a purge whether a session opened there.

use crate::engine::Engine;
use crate::worktree;
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Weak;
use std::time::{Duration, SystemTime};

/// Test executables kept of each name in a live checkout: the last few package selections relink
/// nothing, and going back to an older one is a link, not a rebuild.
pub const KEEP: usize = 3;
/// How long a checkout no session owns sits unbuilt before its build output goes.
pub const UNOWNED_IDLE: Duration = Duration::from_secs(24 * 60 * 60);
/// How long after a close its sweep waits: the agent is killed after the close commits.
const AFTER_CLOSE: Duration = Duration::from_secs(60);

/// What the store holds, read in one short lock: paths as written, resolved by [`Rows::resolve`]
/// once the lock is released, since resolving one is a filesystem call per component.
#[derive(Debug, Default)]
pub struct Rows {
    repos: Vec<PathBuf>,
    /// (worktree, closed, claude, provider_ref) for every session row.
    sessions: Vec<(String, bool, bool, Option<String>)>,
}

/// What the store knows, its paths resolved.
#[derive(Debug, Default)]
pub struct Known {
    pub repos: Vec<PathBuf>,
    /// Checkouts a session that is not closed still works in.
    pub live: HashSet<PathBuf>,
    /// Checkouts whose every session is closed.
    pub closed: HashSet<PathBuf>,
    /// Closed Claude sessions' conversation ids that no open session shares.
    pub dead_refs: HashSet<String>,
}

/// What a sweep removed.
#[derive(Debug, Default)]
pub struct Swept {
    /// Checkouts whose build output went.
    pub purged: Vec<PathBuf>,
    /// Test executables trimmed from live checkouts.
    pub trimmed: usize,
    /// Scratch directories removed.
    pub scratch: Vec<PathBuf>,
    pub freed: u64,
}

pub fn known(conn: &Connection) -> rusqlite::Result<Rows> {
    let repos = conn.prepare_cached("SELECT path FROM projects")?
        .query_map([], |r| r.get::<_, String>(0))?
        .map(|path| path.map(PathBuf::from))
        .collect::<rusqlite::Result<_>>()?;
    let mut statement = conn.prepare_cached("SELECT worktree, state = 'closed', provider = 'claude', provider_ref FROM sessions")?;
    let sessions = statement.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?, r.get::<_, bool>(2)?, r.get::<_, Option<String>>(3)?)))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(Rows { repos, sessions })
}

impl Rows {
    /// Resolve the paths: call it with no store lock held.
    pub fn resolve(self) -> Known {
        let mut known = Known { repos: self.repos, ..Default::default() };
        let mut live_refs = HashSet::new();
        let mut closed_refs = HashSet::new();
        for (worktree, closed, claude, provider_ref) in self.sessions {
            if !worktree.is_empty() {
                let worktree = canon(Path::new(&worktree));
                if closed { known.closed.insert(worktree); } else { known.live.insert(worktree); }
            }
            if let Some(id) = provider_ref.filter(|id| claude && is_uuid(id)) {
                if closed { closed_refs.insert(id); } else { live_refs.insert(id); }
            }
        }
        known.closed.retain(|wt| !known.live.contains(wt));
        known.dead_refs = closed_refs.difference(&live_refs).cloned().collect();
        known
    }
}

/// The checkouts sessions that are not closed work in, now: the purge's last question to the
/// store, asked in one short read for the checkout about to lose its build.
fn live_now(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    conn.prepare_cached("SELECT worktree FROM sessions WHERE state != 'closed' AND worktree != ''")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect()
}

/// Claude Code's scratch space: `<os tmpdir>/claude-<uid>/`.
pub fn scratch_root() -> PathBuf {
    // SAFETY: getuid has no failure mode and touches no memory.
    let uid = unsafe { libc::getuid() };
    std::env::temp_dir().join(format!("claude-{uid}"))
}

/// One pass over every project's checkouts and the scratch root. Filesystem and `/proc` work
/// only: call it with no store lock held. `stop` is asked between checkouts, and `live` just
/// before a purge, with the store as it is then: whether a session that is not closed works in
/// that checkout now. A failed read should answer that one does.
pub fn sweep(known: &Known, scratch: &Path, now: SystemTime, stop: &dyn Fn() -> bool, live: &dyn Fn(&Path) -> bool) -> Swept {
    let mut out = Swept::default();
    for repo in &known.repos {
        let Ok(checkouts) = worktree::list_with_dirty(repo, false) else { continue };
        for (index, checkout) in checkouts.iter().enumerate() {
            if stop() { return out; }
            let path = PathBuf::from(&checkout.path);
            if index == 0 || known.live.contains(&path) {
                if !Procs::read().building_in(&path) {
                    let (count, freed) = trim(&path.join("target"));
                    out.trimmed += count;
                    out.freed += freed;
                }
                continue;
            }
            // A checkout the user made elsewhere is theirs to clean, even one a session worked in.
            if !agent_area(repo, &path) { continue; }
            let idle = if known.closed.contains(&path) { Duration::ZERO } else { UNOWNED_IDLE };
            let dirs = worktree::ignored_build_dirs(&path);
            if dirs.is_empty() || last_build(&dirs).is_some_and(|built| now.duration_since(built).unwrap_or_default() < idle) { continue; }
            let sizes: Vec<u64> = dirs.iter().map(|dir| worktree::dir_size(dir)).collect();
            // Asked after the walk, which can take minutes on a large build: a session opened
            // or a build started meanwhile keeps it.
            if Procs::read().inside(&path) || live(&path) { continue; }
            let mut freed = 0;
            for (dir, size) in dirs.iter().zip(sizes) {
                if std::fs::remove_dir_all(dir).is_ok() { freed += size; }
            }
            if freed > 0 {
                out.purged.push(path);
                out.freed += freed;
            }
        }
    }
    out.freed += remove_scratch(scratch, &known.dead_refs, &mut out.scratch);
    out
}

/// Whether `path` is in a directory agents make checkouts in: Relay's pool and integration area,
/// and Claude Code's `.claude/worktrees/`.
fn agent_area(repo: &Path, path: &Path) -> bool {
    let repo = canon(repo);
    [repo.join(".relay").join("worktrees"), repo.join(".relay").join("integrations"), repo.join(".claude").join("worktrees")]
        .iter()
        .any(|area| path.parent() == Some(area.as_path()))
}

/// The newest change to build output: the build dirs and two levels below them, where any build
/// creates or replaces something (`target/debug/deps`, `target/debug/.fingerprint`).
fn last_build(dirs: &[PathBuf]) -> Option<SystemTime> {
    fn newest(dir: &Path, depth: u32) -> Option<SystemTime> {
        let meta = std::fs::symlink_metadata(dir).ok().filter(|meta| meta.is_dir())?;
        let mut latest = meta.modified().ok();
        if depth > 0 {
            for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                latest = latest.max(newest(&entry.path(), depth - 1));
            }
        }
        latest
    }
    dirs.iter().filter_map(|dir| newest(dir, 2)).max()
}

/// Keep the [`KEEP`] newest test executables of each name in every profile of `target`: cargo
/// names one `<name>-<16 hex>` in `deps/`, and links a new one beside it for every new selection
/// of packages, dependency features or profile. Returns (executables removed, bytes freed).
pub fn trim(target: &Path) -> (usize, u64) {
    use std::os::unix::fs::PermissionsExt;
    let mut removed = 0;
    let mut freed = 0;
    // A `target` or profile that is a link leads to a build other checkouts may be running from,
    // which `building_in` cannot see: the purge does not follow one either (RA-347).
    if !is_dir(target) { return (0, 0); }
    for profile in std::fs::read_dir(target).into_iter().flatten().flatten() {
        let deps = profile.path().join("deps");
        if !is_dir(&profile.path()) || !is_dir(&deps) { continue; }
        let mut groups: HashMap<String, Vec<(SystemTime, PathBuf, u64)>> = HashMap::new();
        for entry in std::fs::read_dir(&deps).into_iter().flatten().flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() || meta.permissions().mode() & 0o111 == 0 { continue; }
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some((stem, hash)) = name.rsplit_once('-') else { continue };
            if stem.is_empty() || hash.len() != 16 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) { continue; }
            let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            groups.entry(stem.to_string()).or_default().push((modified, entry.path(), meta.len()));
        }
        for mut group in groups.into_values() {
            group.sort_by_key(|(modified, ..)| std::cmp::Reverse(*modified));
            for (_, path, len) in group.into_iter().skip(KEEP) {
                if std::fs::remove_file(&path).is_ok() {
                    removed += 1;
                    freed += len;
                    // `<name>-<hash>.d`, appended: a dot in the name is not an extension.
                    let mut dep_info = path.into_os_string();
                    dep_info.push(".d");
                    let _ = std::fs::remove_file(dep_info);
                }
            }
        }
    }
    (removed, freed)
}

/// Remove `<root>/<slug>/<id>/` for every dead conversation id, and a slug left empty by it.
fn remove_scratch(root: &Path, dead: &HashSet<String>, removed: &mut Vec<PathBuf>) -> u64 {
    let mut freed = 0;
    for slug in std::fs::read_dir(root).into_iter().flatten().flatten() {
        let slug = slug.path();
        if !is_dir(&slug) { continue; }
        let mut took = false;
        for entry in std::fs::read_dir(&slug).into_iter().flatten().flatten() {
            let dir = entry.path();
            if !dead.contains(entry.file_name().to_string_lossy().as_ref()) || !is_dir(&dir) { continue; }
            let size = worktree::dir_size(&dir);
            if remove_tree(&dir).is_ok() {
                freed += size;
                took = true;
                removed.push(dir);
            }
        }
        if took {
            let _ = std::fs::remove_dir(&slug);
        }
    }
    freed
}

/// `remove_dir_all`, and once more after making every directory under `dir` writable: agents'
/// test fixtures leave read-only directories behind, which the first attempt cannot empty.
fn remove_tree(dir: &Path) -> std::io::Result<()> {
    fn writable(dir: &Path) {
        use std::os::unix::fs::PermissionsExt;
        let Ok(meta) = std::fs::symlink_metadata(dir) else { return };
        if !meta.is_dir() { return; }
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(meta.permissions().mode() | 0o700));
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            writable(&entry.path());
        }
    }
    std::fs::remove_dir_all(dir).or_else(|_| {
        writable(dir);
        std::fs::remove_dir_all(dir)
    })
}

/// The processes this user can see: what each runs, and where.
struct Procs(Vec<(String, Option<PathBuf>, Option<PathBuf>)>);

impl Procs {
    fn read() -> Procs {
        let mut procs = Vec::new();
        for entry in std::fs::read_dir("/proc").into_iter().flatten().flatten() {
            if !entry.file_name().to_string_lossy().bytes().all(|b| b.is_ascii_digit()) { continue; }
            let dir = entry.path();
            let comm = std::fs::read_to_string(dir.join("comm")).unwrap_or_default().trim().to_string();
            procs.push((comm, std::fs::read_link(dir.join("cwd")).ok(), std::fs::read_link(dir.join("exe")).ok()));
        }
        Procs(procs)
    }

    /// Any process working inside `wt`: an agent, a shell, an editor, a build.
    fn inside(&self, wt: &Path) -> bool {
        self.0.iter().any(|(_, cwd, _)| cwd.as_deref().is_some_and(|cwd| cwd.starts_with(wt)))
    }

    /// A build or a test run in `wt`: cargo or rustc working there, or one of its test
    /// executables running.
    fn building_in(&self, wt: &Path) -> bool {
        let target = wt.join("target");
        self.0.iter().any(|(comm, cwd, exe)| {
            (matches!(comm.as_str(), "cargo" | "rustc") && cwd.as_deref().is_some_and(|cwd| cwd.starts_with(wt)))
                || exe.as_deref().is_some_and(|exe| exe.starts_with(&target) && exe.parent().is_some_and(|dir| dir.ends_with("deps")))
        })
    }
}

/// One sweep at a time; a request while one runs sends it round once more.
static RUNNING: AtomicBool = AtomicBool::new(false);
static AGAIN: AtomicBool = AtomicBool::new(false);
/// A sweep is already waiting out [`AFTER_CLOSE`].
static SOON: AtomicBool = AtomicBool::new(false);

/// Sweep now, on this thread. `None` when another thread's sweep was running; it goes round
/// once more for this request.
pub fn run(engine: &Engine) -> Option<Swept> {
    if RUNNING.swap(true, Ordering::AcqRel) {
        AGAIN.store(true, Ordering::Release);
        return None;
    }
    let mut total = Swept::default();
    loop {
        AGAIN.store(false, Ordering::Release);
        let rows = {
            let conn = engine.store.lock();
            known(&conn)
        };
        let live = |path: &Path| {
            let rows = {
                let conn = engine.store.lock();
                live_now(&conn)
            };
            rows.map_or(true, |rows| rows.iter().any(|wt| canon(Path::new(wt)) == path))
        };
        match rows {
            Ok(rows) => {
                let known = rows.resolve();
                let swept = sweep(&known, &scratch_root(), SystemTime::now(), &|| engine.is_quitting(), &live);
                total.purged.extend(swept.purged);
                total.trimmed += swept.trimmed;
                total.scratch.extend(swept.scratch);
                total.freed += swept.freed;
            }
            Err(error) => tracing::warn!(error = %error, "disk sweep could not read the store"),
        }
        if !AGAIN.load(Ordering::Acquire) || engine.is_quitting() { break; }
    }
    RUNNING.store(false, Ordering::Release);
    if total.freed > 0 {
        tracing::info!(
            freed_mb = total.freed / (1024 * 1024), purged = ?total.purged, trimmed = total.trimmed, scratch = total.scratch.len(),
            "disk sweep",
        );
    }
    Some(total)
}

/// Sweep on a background thread after [`AFTER_CLOSE`], for a close that just committed. Holds
/// only a weak reference, so it never keeps an engine alive.
pub fn soon(engine: Weak<Engine>) {
    if SOON.swap(true, Ordering::AcqRel) { return; }
    let spawned = std::thread::Builder::new().name("disk-sweep".into()).spawn(move || {
        crate::background_priority();
        let deadline = std::time::Instant::now() + AFTER_CLOSE;
        while std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_secs(1));
            if !engine.upgrade().is_some_and(|engine| !engine.is_quitting()) {
                SOON.store(false, Ordering::Release);
                return;
            }
        }
        SOON.store(false, Ordering::Release);
        if let Some(engine) = engine.upgrade() {
            run(&engine);
        }
    });
    if spawned.is_err() { SOON.store(false, Ordering::Release); }
}

fn canon(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn is_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir())
}

/// A conversation id as Claude Code writes one: it names a directory to delete, so nothing else
/// (an empty string, `..`, a path) may pass for one.
fn is_uuid(id: &str) -> bool {
    id.len() == 36 && id.bytes().enumerate().all(|(i, b)| if matches!(i, 8 | 13 | 18 | 23) { b == b'-' } else { b.is_ascii_hexdigit() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    const LIVE: &str = "11111111-2222-4333-8444-555555555555";
    const DEAD: &str = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee";
    const DEAD_TOO: &str = "99999999-8888-4777-8666-555555555555";

    fn ago(secs: u64) -> SystemTime {
        SystemTime::now() - Duration::from_secs(secs)
    }

    fn file(path: &Path, bytes: usize, exec: bool, modified: SystemTime) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, vec![0u8; bytes]).unwrap();
        if exec { std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap(); }
        std::fs::File::options().write(true).open(path).unwrap().set_modified(modified).unwrap();
    }

    fn set_mtime(dir: &Path, modified: SystemTime) {
        std::fs::File::open(dir).unwrap().set_modified(modified).unwrap();
    }

    /// Five links of one test, two of another, and the files that are not test executables.
    fn builds(target: &Path) {
        let deps = target.join("debug/deps");
        for i in 0..5u64 {
            let exe = deps.join(format!("store-{:016x}", i));
            file(&exe, 1000, true, ago(1000 - i * 100));
            file(&exe.with_extension("d"), 10, false, ago(1000 - i * 100));
            file(&deps.join(format!("librelay_core-{:016x}.rlib", i)), 100, false, ago(1000 - i * 100));
        }
        for i in 0..2u64 {
            file(&deps.join(format!("relay-{:016x}", 10 + i)), 1000, true, ago(50));
        }
        file(&deps.join("libserde_derive-0123456789abcdef.so"), 100, true, ago(5000));
        file(&target.join("debug/relay"), 1000, true, ago(5000));
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }

    #[test]
    fn a_trim_keeps_the_newest_test_executables_of_each_name_and_nothing_else_goes() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        builds(&target);
        let (removed, freed) = trim(&target);
        assert_eq!((removed, freed), (2, 2000));
        let left = names(&target.join("debug/deps"));
        for i in 0..2u64 {
            assert!(!left.contains(&format!("store-{:016x}", i)), "{left:?}");
            assert!(!left.contains(&format!("store-{:016x}.d", i)), "{left:?}");
        }
        for i in 2..5u64 {
            assert!(left.contains(&format!("store-{:016x}", i)), "{left:?}");
            assert!(left.contains(&format!("store-{:016x}.d", i)), "{left:?}");
        }
        assert_eq!(left.iter().filter(|n| n.ends_with(".rlib")).count(), 5, "libraries are rebuilds, not links: kept");
        assert_eq!(left.iter().filter(|n| n.starts_with("relay-")).count(), 2);
        assert!(left.contains(&"libserde_derive-0123456789abcdef.so".to_string()));
        assert!(target.join("debug/relay").exists());
        assert_eq!(trim(&target), (0, 0), "a second trim has nothing left to take");
        assert_eq!(trim(&dir.path().join("missing")), (0, 0));
    }

    #[test]
    fn a_trim_never_follows_a_link_to_a_build_elsewhere() {
        let dir = tempfile::tempdir().unwrap();
        let shared = dir.path().join("shared-target");
        builds(&shared);
        let wt = dir.path().join("wt");
        std::fs::create_dir_all(&wt).unwrap();
        std::os::unix::fs::symlink(&shared, wt.join("target")).unwrap();
        assert_eq!(trim(&wt.join("target")), (0, 0));
        let other = dir.path().join("other");
        std::fs::create_dir_all(other.join("target")).unwrap();
        std::os::unix::fs::symlink(shared.join("debug"), other.join("target/debug")).unwrap();
        assert_eq!(trim(&other.join("target")), (0, 0), "nor a profile that is a link");
        assert_eq!(names(&shared.join("debug/deps")).iter().filter(|n| n.starts_with("store-") && !n.ends_with(".d")).count(), 5);
    }

    #[test]
    fn only_a_closed_sessions_scratch_goes_read_only_fixtures_included() {
        let root = tempfile::tempdir().unwrap();
        let slug = root.path().join("-home-me-repo--relay-worktrees-a");
        std::fs::create_dir_all(slug.join(LIVE).join("scratchpad")).unwrap();
        file(&slug.join(DEAD).join("scratchpad/head-target/big"), 4096, false, ago(10));
        let other = root.path().join("-home-me-repo--relay-worktrees-b");
        let locked = other.join(DEAD_TOO).join("scratchpad/fixture/ro");
        file(&locked.join("f"), 10, false, ago(10));
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
        std::fs::create_dir_all(root.path().join("bundled-skills").join("2.1.302")).unwrap();
        std::fs::write(root.path().join(DEAD), "a file named like an id is not a scratch directory").unwrap();

        let dead: HashSet<String> = [DEAD, DEAD_TOO].into_iter().map(String::from).collect();
        let mut removed = Vec::new();
        let freed = remove_scratch(root.path(), &dead, &mut removed);
        assert!(freed >= 4096, "{freed}");
        assert_eq!(removed.len(), 2, "{removed:?}");
        assert!(!slug.join(DEAD).exists());
        assert!(slug.join(LIVE).join("scratchpad").is_dir(), "a live session's scratch stays");
        assert!(!other.exists(), "a slug the sweep emptied goes with it");
        assert!(root.path().join("bundled-skills/2.1.302").is_dir(), "what is not a session's stays");
        assert!(root.path().join(DEAD).is_file());
        assert_eq!(remove_scratch(&root.path().join("missing"), &dead, &mut removed), 0);
    }

    #[test]
    fn only_a_well_formed_id_can_name_a_scratch_directory() {
        assert!(is_uuid(DEAD));
        assert!(!is_uuid(""));
        assert!(!is_uuid(".."));
        assert!(!is_uuid("aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeee/"));
        assert!(!is_uuid("../aaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee"));
        assert!(!is_uuid("aaaaaaaaabbbbb4ccc-8ddd-eeeeeeeeeeee"));
    }

    #[test]
    fn the_store_says_which_checkouts_and_scratch_belong_to_closed_sessions() {
        let store = crate::store::Store::open_memory().unwrap();
        let conn = store.lock();
        conn.execute_batch(
            "INSERT INTO workspaces (id, path, name, created_at, updated_at) VALUES (1, '/w', 'w', '', '');
             INSERT INTO projects (id, workspace_id, path, name, created_at, updated_at) VALUES (1, 1, '/w/repo', 'repo', '', '');",
        ).unwrap();
        let session = |name: &str, worktree: &str, provider: &str, state: &str, provider_ref: Option<&str>| {
            conn.execute(
                "INSERT INTO sessions (name, project_id, provider, worktree, state, provider_ref, created_at, updated_at) VALUES (?1, 1, ?2, ?3, ?4, ?5, '', '')",
                rusqlite::params![name, provider, worktree, state, provider_ref],
            ).unwrap();
        };
        session("gone", "/w/repo/.relay/worktrees/gone", "claude", "closed", Some(DEAD));
        session("here", "/w/repo/.relay/worktrees/here", "claude", "running", Some(LIVE));
        // A checkout one session closed on and another still works in is live.
        session("old", "/w/repo/.relay/worktrees/here", "claude", "closed", Some(DEAD_TOO));
        // Resumed: the open session carries on the closed one's conversation.
        session("resumed", "/w/repo/.relay/worktrees/again", "claude", "restorable", Some(DEAD_TOO));
        session("codex", "/w/repo/.relay/worktrees/codex", "codex", "closed", Some("12345678-1234-4234-8234-123456789012"));
        session("odd", "", "claude", "closed", Some("../../etc"));

        let known = known(&conn).unwrap().resolve();
        assert_eq!(known.repos, vec![PathBuf::from("/w/repo")]);
        let closed: HashSet<PathBuf> = ["/w/repo/.relay/worktrees/gone", "/w/repo/.relay/worktrees/codex"].into_iter().map(PathBuf::from).collect();
        assert_eq!(known.closed, closed);
        assert!(known.live.contains(Path::new("/w/repo/.relay/worktrees/here")));
        assert!(known.live.contains(Path::new("/w/repo/.relay/worktrees/again")));
        assert_eq!(known.dead_refs, [DEAD.to_string()].into_iter().collect::<HashSet<_>>());
    }

    fn git(dir: &Path, args: &[&str]) {
        let out = std::process::Command::new("git").arg("-C").arg(dir)
            .args(["-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid"]).args(args).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }

    /// The idle clock reads the build dirs' own times: age them after writing into them.
    fn age(wt: &Path, modified: SystemTime) {
        for dir in ["target/debug/deps", "target/debug", "target"] {
            set_mtime(&wt.join(dir), modified);
        }
    }

    #[test]
    fn a_sweep_purges_closed_and_abandoned_checkouts_and_trims_live_ones() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("repo")).unwrap();
        let repo = canon(&dir.path().join("repo"));
        git(&repo, &["init", "-q", "-b", "main"]);
        std::fs::write(repo.join(".gitignore"), "target/\n").unwrap();
        // A folder named like build output that the project tracks is not build output.
        std::fs::create_dir_all(repo.join("build")).unwrap();
        std::fs::write(repo.join("build/release.sh"), "#!/bin/sh\n").unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-q", "-m", "init"]);
        let at = |area: &str, name: &str| {
            let path = repo.join(area).join(name);
            git(&repo, &["worktree", "add", "-q", "-b", name, path.to_str().unwrap()]);
            builds(&path.join("target"));
            canon(&path)
        };
        let closed = at(".relay/worktrees", "closed");
        let live = at(".relay/worktrees", "live");
        let abandoned = at(".claude/worktrees", "agent-old");
        let recent = at(".claude/worktrees", "agent-new");
        let users = at("..", "users-own");
        builds(&repo.join("target"));
        for wt in [&closed, &live, &abandoned, &users] {
            age(wt, ago(2 * 24 * 60 * 60));
        }

        // A session worked in the user's own checkout and closed: `session.close` keeps a checkout
        // outside the pool, and its builds stay with it.
        let known = Known {
            repos: vec![repo.clone()],
            live: [live.clone()].into_iter().collect(),
            closed: [closed.clone(), users.clone()].into_iter().collect(),
            dead_refs: HashSet::new(),
        };
        let swept = sweep(&known, &dir.path().join("no-scratch"), SystemTime::now(), &|| false, &|_| false);

        let mut purged = swept.purged.clone();
        purged.sort();
        let mut want = vec![closed.clone(), abandoned.clone()];
        want.sort();
        assert_eq!(purged, want);
        assert!(!closed.join("target").exists());
        assert!(closed.join("build/release.sh").is_file(), "tracked files stay");
        assert!(closed.join(".gitignore").is_file(), "the checkout itself stays");
        assert!(!abandoned.join("target").exists());
        assert!(recent.join("target/debug/deps").is_dir(), "an agent's checkout built in the last day stays");
        assert!(users.join("target/debug/deps").is_dir(), "a checkout outside the agent areas is the user's");
        assert_eq!(users.join("target/debug/deps").read_dir().unwrap().count(), names(&recent.join("target/debug/deps")).len());
        // The live checkout and the primary keep their builds, less the older links.
        for wt in [&live, &repo] {
            let left = names(&wt.join("target/debug/deps"));
            assert_eq!(left.iter().filter(|n| n.starts_with("store-") && !n.ends_with(".d")).count(), KEEP, "{left:?}");
            assert_eq!(left.iter().filter(|n| n.ends_with(".rlib")).count(), 5);
        }
        assert_eq!(swept.trimmed, 4);
        assert!(swept.freed >= 4 * 1000);

        let again = sweep(&known, &dir.path().join("no-scratch"), SystemTime::now(), &|| false, &|_| false);
        assert!(again.purged.is_empty() && again.trimmed == 0, "{again:?}");
        let stopped = sweep(&Known { closed: [recent.clone()].into_iter().collect(), ..known }, &dir.path().join("no-scratch"), SystemTime::now(), &|| true, &|_| false);
        assert!(stopped.purged.is_empty(), "a quitting engine stops the sweep");
        assert!(recent.join("target").is_dir());
    }

    /// The snapshot is from the start of a pass that can take minutes: a session opened on a
    /// closed checkout since then keeps its build.
    #[test]
    fn a_session_opened_during_the_pass_keeps_its_checkouts_build() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("repo")).unwrap();
        let repo = canon(&dir.path().join("repo"));
        git(&repo, &["init", "-q", "-b", "main"]);
        std::fs::write(repo.join(".gitignore"), "target/\n").unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-q", "-m", "init"]);
        let path = repo.join(".relay/worktrees/reopened");
        git(&repo, &["worktree", "add", "-q", "-b", "reopened", path.to_str().unwrap()]);
        builds(&path.join("target"));
        let path = canon(&path);
        let known = Known { repos: vec![repo.clone()], closed: [path.clone()].into_iter().collect(), ..Default::default() };
        let asked = std::cell::RefCell::new(Vec::new());
        let swept = sweep(&known, &dir.path().join("no-scratch"), SystemTime::now(), &|| false, &|wt| {
            asked.borrow_mut().push(wt.to_path_buf());
            true
        });
        assert!(swept.purged.is_empty(), "{swept:?}");
        assert_eq!(asked.into_inner(), vec![path.clone()], "the store is asked about the checkout it would purge");
        assert!(path.join("target/debug/deps").is_dir());
        let swept = sweep(&known, &dir.path().join("no-scratch"), SystemTime::now(), &|| false, &|_| false);
        assert_eq!(swept.purged, vec![path.clone()]);
    }

    #[test]
    fn nothing_is_purged_or_trimmed_from_under_a_process_working_there() {
        let dir = tempfile::tempdir().unwrap();
        let wt = canon(dir.path());
        let me = Procs(vec![
            ("claude".into(), Some(wt.join("crates/relay-core")), Some(PathBuf::from("/usr/bin/claude"))),
        ]);
        assert!(me.inside(&wt));
        assert!(!me.building_in(&wt), "an agent at rest is not a build");
        let cargo = Procs(vec![("cargo".into(), Some(wt.clone()), Some(PathBuf::from("/usr/bin/cargo")))]);
        assert!(cargo.building_in(&wt));
        let test = Procs(vec![("store-0123456".into(), Some(PathBuf::from("/")), Some(wt.join("target/debug/deps/store-0123456789abcdef")))]);
        assert!(test.building_in(&wt), "a test executable running out of deps");
        let app = Procs(vec![("relay-native".into(), Some(PathBuf::from("/")), Some(wt.join("target/debug/relay-native")))]);
        assert!(!app.building_in(&wt), "the app run from target/debug does not stop a trim of deps");
        assert!(!app.inside(&wt));
    }
}
