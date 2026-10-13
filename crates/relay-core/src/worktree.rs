//! Worktrees (SPEC §1, §8): reads through gix, mutations through the `git` binary. The pool
//! lives at `<repo>/.relay/worktrees/<name>` on branch `relay/<name>`; `.relay/` is added to
//! `.git/info/exclude` so it never shows up in status.

use crate::private::read_regular_through_links;
use anyhow::{anyhow, Context, Result};
use relay_bus::types::{FileStatus, Worktree};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Directories that are build output and safe to purge before removing a worktree
/// (SPEC §8: "gradlew clean / target purge — the 73GB problem, closed").
pub const BUILD_DIRS: &[&str] = &["target", "build", ".gradle", "app/build", "node_modules/.cache", "dist"];

pub fn pool_dir(repo: &Path) -> PathBuf {
    repo.join(".relay").join("worktrees")
}

pub fn pooled_path(repo: &Path, name: &str) -> PathBuf {
    pool_dir(repo).join(name)
}

pub fn branch_for(name: &str) -> String {
    format!("relay/{name}")
}

/// The longest any one git subprocess here may run. Generous — a checkout through LFS
/// filters, a signing prompt — but never unbounded (D144).
const GIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);

fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo).args(args);
    let out = crate::proc::output_with_timeout(&mut command, GIT_TIMEOUT)
        .with_context(|| format!("running git {}", args.join(" ")))?
        .ok_or_else(|| anyhow!("git {} did not finish within {} s", args.join(" "), GIT_TIMEOUT.as_secs()))?;
    if !out.status.success() {
        return Err(anyhow!("git {} failed: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Run a mutating system-git operation. Read paths stay in gix-backed helpers.
pub fn git_mutate(repo: &Path, args: &[&str]) -> Result<String> { git(repo, args) }

/// Make sure `.relay/` is excluded in this repo without touching its tracked `.gitignore`.
/// `.codex/hooks.json` is not: a project may commit its own, so `hooks::install_codex` excludes
/// it only once Relay's handlers are in an untracked one.
pub fn ensure_excluded(repo: &Path) -> Result<()> {
    exclude_paths(repo, &[".relay/", ".claude/settings.local.json"])
}

/// Same, for paths Relay writes into a checkout on demand (materialized skill folders).
pub fn exclude_paths(repo: &Path, entries: &[&str]) -> Result<()> {
    // One read-modify-write at a time: skills call this under their own lock, the hook
    // adapters under `hooks::writes()`, and both edit the one `info/exclude` a repository's
    // worktrees share.
    static EXCLUDE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _one = EXCLUDE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let git_dir = git_dir_of(repo)?;
    let info = git_dir.join("info");
    std::fs::create_dir_all(&info)?;
    let exclude = info.join("exclude");
    // Under the lock above, and `hooks::writes()` for the adapters: a FIFO planted here must
    // not hang the read, nor the write after it. A file that cannot be read is not replaced
    // by one holding only these entries.
    let cur = match read_regular_through_links(&exclude) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error).with_context(|| format!("reading {}", exclude.display())),
    };
    let mut missing = Vec::new();
    for entry in entries.iter().copied() {
        if !cur.lines().any(|line| line.trim() == entry.trim_end_matches('/'))
            && !cur.lines().any(|line| line.trim() == entry)
        {
            missing.push(entry);
        }
    }
    if !missing.is_empty() {
        let mut contents = cur;
        if !contents.is_empty() && !contents.ends_with('\n') { contents.push('\n'); }
        for entry in missing {
            contents.push_str(entry);
            contents.push('\n');
        }
        std::fs::write(&exclude, contents)?;
    }
    Ok(())
}

/// The common `.git` dir of a repo (works from the primary checkout).
pub fn git_dir_of(repo: &Path) -> Result<PathBuf> {
    let r = gix::open(repo).with_context(|| format!("opening {}", repo.display()))?;
    Ok(r.common_dir().to_path_buf())
}

/// Uncommitted changes of any kind — modified, staged, or untracked (an agent's new file is
/// a change too). `gix::Repository::is_dirty` ignores untracked files, hence this.
pub fn is_dirty(r: &gix::Repository) -> bool {
    // A failed scan cannot establish that deleting/switching a checkout is safe. An untracked
    // directory is one entry, still a change, and not a walk listing every file under it.
    r.workdir().is_none_or(|root| {
        status_files_with(root, Untracked::Directories).map_or(true, |files| !files.is_empty())
    })
}

/// How often a background status may take the index lock to save the stat data it refreshed.
const INDEX_REFRESH_EVERY: std::time::Duration = std::time::Duration::from_secs(60);
/// An index written this recently means someone is running git in the checkout right now.
const INDEX_BUSY: std::time::Duration = std::time::Duration::from_secs(5);
static INDEX_REFRESHED: std::sync::Mutex<Option<std::collections::HashMap<PathBuf, std::time::Instant>>> =
    std::sync::Mutex::new(None);

/// The git dir of the checkout at `root`: `.git` itself, or for a linked worktree the slot its
/// `.git` file points at, where its own index and HEAD live.
pub(crate) fn checkout_git_dir(root: &Path) -> Option<PathBuf> {
    let dotgit = root.join(".git");
    if dotgit.is_dir() { return Some(dotgit); }
    let pointer = read_regular_through_links(&dotgit).ok()?;
    let gitdir = PathBuf::from(pointer.trim().strip_prefix("gitdir:")?.trim());
    Some(if gitdir.is_absolute() { gitdir } else { root.join(gitdir) })
}

/// The index file of the checkout at `root`, a linked worktree's included.
fn index_file(root: &Path) -> Option<PathBuf> {
    Some(checkout_git_dir(root)?.join("index"))
}

/// Whether this status may take git's optional index lock. Holding it for the length of a scan
/// made a person's own `git add` or `git commit` in the same checkout fail on `index.lock`,
/// and Relay scans after every burst of file changes (RA-131). Without the lock, though, git
/// cannot save the stat data it refreshed, and touched-but-unchanged LFS assets are rehashed
/// by every scan. So: at most once a minute per checkout, and never while git is visibly busy
/// there (its lock is held, or it wrote the index a moment ago).
fn may_lock_index(root: &Path) -> bool {
    let Some(index) = index_file(root) else { return false };
    if index.with_extension("lock").exists()
        || std::fs::metadata(&index).and_then(|meta| meta.modified())
            .is_ok_and(|at| at.elapsed().is_ok_and(|age| age < INDEX_BUSY))
    {
        return false;
    }
    let mut refreshed = INDEX_REFRESHED.lock().unwrap_or_else(|poison| poison.into_inner());
    let refreshed = refreshed.get_or_insert_with(Default::default);
    refreshed.retain(|_, at| at.elapsed() < INDEX_REFRESH_EVERY);
    if refreshed.contains_key(root) { return false; }
    refreshed.insert(root.to_path_buf(), std::time::Instant::now());
    true
}

/// Git persists refreshed stat data with its index locking. Dropping gix's status
/// outcome rehashes touched-but-unchanged LFS assets on every refresh; writing an
/// old index snapshot ourselves could lose concurrent staging.
pub fn status_files(root: &Path) -> Result<Vec<FileStatus>> {
    status_files_with(root, Untracked::All)
}

/// How `git status` lists untracked files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Untracked {
    /// Each file on its own (`--untracked-files=all`): what per-file badges and diffs need.
    All,
    /// A directory holding nothing tracked as one entry, `dir/` (`--untracked-files=normal`):
    /// an un-ignored `venv/` or `node_modules/` is one change, not every file in it (RA-205).
    Directories,
}

pub fn status_files_with(root: &Path, untracked: Untracked) -> Result<Vec<FileStatus>> {
    let mut command = Command::new("git");
    command.arg("-C").arg(root)
        .args(["status", "--porcelain=v1", "-z", match untracked {
            Untracked::All => "--untracked-files=all",
            Untracked::Directories => "--untracked-files=normal",
        }])
        .env("GIT_OPTIONAL_LOCKS", if may_lock_index(root) { "1" } else { "0" });
    let output = crate::proc::output_with_timeout(&mut command, std::time::Duration::from_secs(10))?
        .ok_or_else(|| anyhow!("Git status timed out after 10 seconds; check repository filters and retry"))?;
    if !output.status.success() {
        return Err(anyhow!("Git status failed: {}", String::from_utf8_lossy(&output.stderr).trim()));
    }
    parse_status(&output.stdout)
}

/// [`status_files_with`] for a checkout about to be removed. One that lost its own `.git` (a
/// removal or a crash half done) is not a checkout any more: `git status` there would walk up
/// and answer for the primary that holds it. What is left in it is untracked, so its entries
/// are listed as such, a directory as one, build output apart — never the primary's changes.
pub fn checkout_changes(root: &Path, untracked: Untracked) -> Result<Vec<FileStatus>> {
    if root.join(".git").exists() {
        return status_files_with(root, untracked);
    }
    let mut files = Vec::new();
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if crate::watch::generated_name(&name) { continue; }
        let dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
        files.push(FileStatus { path: if dir { format!("{name}/") } else { name }, index: String::new(), worktree: "?".into(), renamed_from: None });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

fn parse_status(output: &[u8]) -> Result<Vec<FileStatus>> {
    let mut records = output.split(|byte| *byte == 0).filter(|record| !record.is_empty());
    let mut files = Vec::new();
    while let Some(record) = records.next() {
        if record.len() < 4 || record[2] != b' ' {
            return Err(anyhow!("Malformed Git status record"));
        }
        let untracked = &record[..2] == b"??";
        let index = if untracked || record[0] == b' ' { String::new() } else { (record[0] as char).to_string() };
        let worktree = if record[1] == b' ' { String::new() } else { (record[1] as char).to_string() };
        let renamed_from = if record[..2].iter().any(|status| matches!(status, b'R' | b'C')) {
            Some(String::from_utf8_lossy(records.next().ok_or_else(|| anyhow!("Missing Git rename source"))?).into_owned())
        } else { None };
        files.push(FileStatus {
            path: String::from_utf8_lossy(&record[3..]).into_owned(),
            index, worktree, renamed_from,
        });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

fn head_of(r: &gix::Repository) -> (String, String) {
    let branch = r.head_name().ok().flatten().map(|n| n.shorten().to_string()).unwrap_or_else(|| "HEAD".into());
    let head = r.head_id().map(|id| id.to_string()).unwrap_or_default();
    (branch, head)
}

/// All worktrees of a repo: the primary first, then linked ones. `session` is filled by the
/// caller (the store knows who owns what); `disk_mb` only when asked (it walks the tree).
pub fn list(repo: &Path) -> Result<Vec<Worktree>> {
    list_with_dirty(repo, true)
}

/// Worktree metadata with optional dirty scans. Hot UI queries skip untracked-file scans in every
/// linked checkout and project the selected checkout's dirty state from their one `git.status`.
pub fn list_with_dirty(repo: &Path, include_dirty: bool) -> Result<Vec<Worktree>> {
    let r = gix::open(repo).with_context(|| format!("opening {}", repo.display()))?;
    let mut out = Vec::new();
    let (branch, head) = head_of(&r);
    let dirty = include_dirty && is_dirty(&r);
    let primary = r.workdir().map(Path::to_path_buf).unwrap_or_else(|| repo.to_path_buf());
    out.push(Worktree { path: canon(&primary), branch, head, session: None, dirty, disk_mb: None });
    for proxy in r.worktrees().context("listing worktrees")? {
        let Ok(base) = proxy.base() else { continue };
        if !base.exists() { continue; }
        match proxy.into_repo() {
            Ok(wr) => {
                let (branch, head) = head_of(&wr);
                let dirty = include_dirty && is_dirty(&wr);
                out.push(Worktree { path: canon(&base), branch, head, session: None, dirty, disk_mb: None });
            }
            Err(_) => out.push(Worktree { path: canon(&base), branch: "?".into(), head: String::new(), session: None, dirty: false, disk_mb: None }),
        }
    }
    Ok(out)
}

/// Validate worktree membership without dirty-scanning every checkout, and without opening
/// the repository: every project-scoped file and git op calls this to check its root, and a
/// `gix::open` plus a worktree enumeration was a third of a tree listing (PERF §1.6). The
/// primary checkout is the project path; a linked checkout's `.git` is a file pointing at its
/// slot under the common dir's `worktrees/`, and that slot's `gitdir` file must point back at
/// this checkout, which is what `git worktree list` reads. Without the second half a `cp -r` of
/// a worktree passed, and git ops there ran against the original's index and HEAD (RA-346).
/// A checkout moved by hand, whose slot still names the old place, is an error asking for
/// `git worktree repair` rather than "not a worktree". Callers hold locks, so the pointer and
/// slot files are read as regular files only: a FIFO at `<checkout>/.git` is no worktree, not a
/// hang.
pub fn contains(repo: &Path, candidate: &Path) -> Result<bool> {
    let want = canon(candidate);
    if canon(repo) == want { return Ok(true); }
    let Ok(pointer) = read_regular_through_links(&Path::new(&want).join(".git")) else { return Ok(false) };
    let Some(gitdir) = pointer.trim().strip_prefix("gitdir:") else { return Ok(false) };
    let gitdir = PathBuf::from(gitdir.trim());
    let gitdir = if gitdir.is_absolute() { gitdir } else { Path::new(&want).join(gitdir) };
    let slots = canon(&common_dir(repo)?.join("worktrees"));
    if !canon(&gitdir).starts_with(&slots) { return Ok(false); }
    let Ok(back) = read_regular_through_links(&gitdir.join("gitdir")) else { return Ok(false) };
    // `<checkout>/.git`, absolute, or relative to the slot (`worktree.useRelativePaths`).
    let back = PathBuf::from(back.trim());
    let back = if back.is_absolute() { back } else { gitdir.join(back) };
    let named = back.parent().map(canon).unwrap_or_default();
    if named == want { return Ok(true); }
    // The checkout the slot names still points at the slot: this is a copy of it.
    let slot = canon(&gitdir);
    let still_there = read_regular_through_links(&back).ok()
        .and_then(|pointer| pointer.trim().strip_prefix("gitdir:").map(|p| PathBuf::from(p.trim())))
        .is_some_and(|p| canon(&if p.is_absolute() { p } else { Path::new(&named).join(p) }) == slot);
    if still_there { return Ok(false); }
    Err(anyhow!("{want} uses the worktree slot of {named}, which is gone; if the checkout was moved by hand, run `git worktree repair` in it"))
}

/// The common git dir of `repo` from its `.git` alone; gix only when that is not a plain
/// directory or pointer file.
fn common_dir(repo: &Path) -> Result<PathBuf> {
    let dotgit = repo.join(".git");
    if dotgit.is_dir() { return Ok(dotgit); }
    if let Ok(pointer) = read_regular_through_links(&dotgit) {
        if let Some(gitdir) = pointer.trim().strip_prefix("gitdir:") {
            let gitdir = PathBuf::from(gitdir.trim());
            let gitdir = if gitdir.is_absolute() { gitdir } else { repo.join(gitdir) };
            if let Ok(common) = read_regular_through_links(&gitdir.join("commondir")) {
                let common = PathBuf::from(common.trim());
                return Ok(if common.is_absolute() { common } else { gitdir.join(common) });
            }
            return Ok(gitdir);
        }
    }
    git_dir_of(repo)
}

fn canon(p: &Path) -> String {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf()).display().to_string()
}

/// Where a new worktree's branch comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Start {
    /// The branch exists already: check it out as it is.
    Existing,
    /// Create the branch, at this commit or ref (`None`: HEAD, or an unborn base).
    New(Option<String>),
}

/// `git worktree add -b <branch> <path> [<from>]`; if the branch exists already, check it out
/// instead of creating it. A caller that already knows which calls [`create_at`].
pub fn create(repo: &Path, path: &Path, branch: &str, from: Option<&str>) -> Result<Worktree> {
    let exists = gix::open(repo)?.find_reference(format!("refs/heads/{branch}").as_str()).is_ok();
    let start = if exists { Start::Existing } else { Start::New(from.map(str::to_string)) };
    create_at(repo, path, branch, &start)
}

/// `git worktree add` for a branch whose [`Start`] the caller has settled (RA-640).
pub fn create_at(repo: &Path, path: &Path, branch: &str, start: &Start) -> Result<Worktree> {
    ensure_excluded(repo)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let path_s = path.display().to_string();
    let args = match start {
        Start::Existing => vec!["worktree", "add", &path_s, branch],
        Start::New(from) => {
            let mut args = vec!["worktree", "add", "-b", branch, &path_s];
            if let Some(f) = from { args.push(f); }
            args
        }
    };
    let mut command = Command::new("git");
    command.arg("-C").arg(repo).args(args).env("GIT_TERMINAL_PROMPT", "0");
    let output = crate::proc::output_with_timeout(&mut command, std::time::Duration::from_secs(120))?
        .ok_or_else(|| anyhow!("Worktree creation timed out after 120 seconds; partial files are preserved at {}", path.display()))?;
    if !output.status.success() {
        return Err(anyhow!("Worktree creation failed at {}: {}", path.display(), String::from_utf8_lossy(&output.stderr).trim()));
    }
    let all = list_with_dirty(repo, false)?;
    let want = canon(path);
    all.into_iter().find(|w| w.path == want).ok_or_else(|| anyhow!("worktree {} not listed after add", path.display()))
}

/// How many threads share a disk walk.
///
/// A `target/` or `node_modules/` tree is hundreds of thousands of `statx` calls, and the walk
/// is latency-bound on the kernel rather than on this process, so a few workers are worth far
/// more than their scheduling cost. The cap is deliberately well under the core count and the
/// helpers run niced: this is background work, and taking every core for a short burst is a
/// worse neighbour to a game or a build than taking a few for slightly longer.
const WALK_THREADS: usize = 4;

/// Bytes under `dir` (no symlink following).
///
/// Directories are handed out from a shared stack, so every worker keeps finding new subtrees
/// instead of waiting on whoever drew the deep one.
pub fn dir_size(dir: &Path) -> u64 {
    dir_size_all(std::slice::from_ref(&dir.to_path_buf()))
}

/// Size of the purge-able build dirs inside a worktree. One walk over all of them together:
/// `target/` is usually far bigger than the rest put together, so walking them in sequence
/// means waiting for it alone.
pub fn build_size(wt: &Path) -> u64 {
    let roots: Vec<PathBuf> = build_dirs(wt).collect();
    walk(&roots, &[]).0
}

/// (all bytes, build-output bytes) under a worktree, in one walk. Build output is exactly what
/// [`purge_build`] would delete, so the resources panel's figure predicts what a purge frees.
pub fn disk_usage(wt: &Path) -> (u64, u64) {
    let build: Vec<PathBuf> = build_dirs(wt).collect();
    walk(std::slice::from_ref(&wt.to_path_buf()), &build)
}

/// The [`BUILD_DIRS`] of `wt` that are real directories inside it. Every component is checked
/// without following symlinks: `node_modules` linked to a shared install must not have that
/// install's `.cache` purged or counted as this worktree's (RA-347).
fn build_dirs(wt: &Path) -> impl Iterator<Item = PathBuf> + '_ {
    BUILD_DIRS.iter().filter_map(move |d| {
        let mut at = wt.to_path_buf();
        for part in Path::new(d).components() {
            at.push(part);
            if !std::fs::symlink_metadata(&at).is_ok_and(|meta| meta.is_dir()) { return None; }
        }
        Some(at)
    })
}

fn dir_size_all(roots: &[PathBuf]) -> u64 {
    walk(roots, &[]).0
}

/// Bytes under `roots`, and how many of them lie under `build` (directories met on the way).
fn walk(roots: &[PathBuf], build: &[PathBuf]) -> (u64, u64) {
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::sync::{Arc, Condvar, Mutex};

    let pending: Vec<(PathBuf, bool)> = roots.iter().filter(|r| r.is_dir()).map(|r| (r.clone(), build.contains(r))).collect();
    if pending.is_empty() {
        return (0, 0);
    }
    let build_total = Arc::new(AtomicU64::new(0));
    let build: Arc<[PathBuf]> = build.into();
    let total = Arc::new(AtomicU64::new(0));
    // `stack` plus `busy` (workers holding a directory) is the whole termination condition:
    // the walk is done when nothing is queued and nobody is still producing.
    let stack = Arc::new((Mutex::new(pending), Condvar::new()));
    let busy = Arc::new(AtomicUsize::new(0));
    let threads = WALK_THREADS.min(std::thread::available_parallelism().map_or(2, |n| n.get()));

    let worker = {
        let total = total.clone();
        let build_total = build_total.clone();
        let build = build.clone();
        let stack = stack.clone();
        let busy = busy.clone();
        move || {
            let (queue, wake) = &*stack;
            loop {
                let dir = {
                    let mut queue = queue.lock().unwrap_or_else(|p| p.into_inner());
                    loop {
                        if let Some(dir) = queue.pop() {
                            busy.fetch_add(1, Ordering::AcqRel);
                            break Some(dir);
                        }
                        if busy.load(Ordering::Acquire) == 0 {
                            break None;
                        }
                        queue = wake.wait(queue).unwrap_or_else(|p| p.into_inner());
                    }
                };
                let Some((dir, in_build)) = dir else {
                    wake.notify_all();
                    return;
                };
                let mut bytes = 0u64;
                let mut found = Vec::new();
                if let Ok(entries) = std::fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        let Ok(meta) = entry.metadata() else { continue };
                        if meta.file_type().is_symlink() {
                            continue;
                        }
                        if meta.is_dir() {
                            let path = entry.path();
                            let child_build = in_build || build.contains(&path);
                            found.push((path, child_build));
                        } else {
                            bytes += meta.len();
                        }
                    }
                }
                if bytes > 0 {
                    total.fetch_add(bytes, Ordering::Relaxed);
                    if in_build { build_total.fetch_add(bytes, Ordering::Relaxed); }
                }
                let mut queue = queue.lock().unwrap_or_else(|p| p.into_inner());
                queue.append(&mut found);
                busy.fetch_sub(1, Ordering::AcqRel);
                drop(queue);
                wake.notify_all();
            }
        }
    };

    // The helpers step down; the calling thread does not, because it belongs to a pool that
    // will run ordinary requests again as soon as this returns.
    let mut handles = Vec::with_capacity(threads.saturating_sub(1));
    for _ in 1..threads {
        let worker = worker.clone();
        let spawned = std::thread::Builder::new()
            .name("relay-disk-walk".into())
            .spawn(move || {
                crate::background_priority();
                worker()
            });
        match spawned {
            Ok(handle) => handles.push(handle),
            Err(_) => break,
        }
    }
    worker();
    for handle in handles {
        let _ = handle.join();
    }
    (total.load(Ordering::Acquire), build_total.load(Ordering::Acquire))
}

/// Delete build output. Returns bytes freed.
pub fn purge_build(wt: &Path) -> u64 {
    let mut freed = 0;
    for p in build_dirs(wt).collect::<Vec<_>>() {
        freed += dir_size(&p);
        let _ = std::fs::remove_dir_all(&p);
    }
    freed
}

/// The directories Relay creates checkouts in: the session pool and the integration area.
fn managed_dirs(repo: &Path) -> [PathBuf; 2] {
    [pool_dir(repo), repo.join(".relay").join("integrations")]
}

/// Remove a linked worktree (never the primary). Returns bytes freed. The branch is kept.
/// A rescue ref [`remove_rescuing`] had to make is logged.
pub fn remove(repo: &Path, path: &Path, purge: bool) -> Result<u64> {
    remove_rescuing(repo, path, purge).map(|removed| removed.freed)
}

/// What [`remove_rescuing`] did.
pub struct Removed {
    /// Bytes freed.
    pub freed: u64,
    /// The ref pinning a detached HEAD's commits that no branch or tag held.
    pub rescued: Option<String>,
}

/// Remove a linked worktree (never the primary). The branch is kept, and so are the commits of
/// a detached HEAD no branch or tag contains: they are pinned under `refs/relay/rescue/` first
/// ([`rescue_detached`]), since the checkout's reflog goes with it.
///
/// `path` must be a linked worktree git lists for `repo`, or a direct child of one of Relay's
/// own checkout areas (a pooled checkout half-deleted, whose admin entry may already be pruned).
/// Anything else is refused before a byte is touched: this used to purge build directories
/// under, and then `rm -rf`, whatever path it was handed (RA-017), and until the child rule a
/// directory inside a live pooled checkout (`.relay/worktrees/<name>/src`) still passed. The
/// plain `rm -rf` fallback is likewise kept to Relay's own areas; a registered checkout
/// elsewhere that git will not remove (locked, say) stays where it is.
pub fn remove_rescuing(repo: &Path, path: &Path, purge: bool) -> Result<Removed> {
    let (path_c, managed) = removable(repo, path)?;
    let mut freed = 0;
    let mut rescued = None;
    if path.exists() {
        rescued = rescue_detached(repo, Path::new(&path_c))?;
        if let Some(rescue) = &rescued {
            tracing::warn!(worktree = %path_c, %rescue, "the checkout's detached HEAD was on no branch; pinned its commits before removing it");
        }
        if purge { freed += purge_build(path); }
        freed += dir_size(path);
        git(repo, &["worktree", "remove", "--force", &path_c])
            .or_else(|e| {
                if !managed {
                    return Err(e);
                }
                // a half-deleted worktree: remove the dir ourselves and prune
                std::fs::remove_dir_all(path).map(|_| String::new()).map_err(|io| anyhow!("{e}; and rm -rf failed: {io}"))
            })?;
    }
    let _ = git(repo, &["worktree", "prune"]);
    Ok(Removed { freed, rescued })
}

/// Whether [`remove_rescuing`] would take `path` at all, without touching it: its canonical
/// spelling, and whether it is in one of Relay's own areas. A caller with its own checks to
/// make first (`worktree.remove`'s dirty check) refuses an unremovable path before them.
pub fn removable(repo: &Path, path: &Path) -> Result<(String, bool)> {
    if !path.is_absolute() {
        return Err(anyhow!("{} is not an absolute path", path.display()));
    }
    let repo_c = canon(repo);
    let path_c = canon(path);
    if repo_c == path_c {
        return Err(anyhow!("refusing to remove the primary checkout"));
    }
    let managed = managed_dirs(repo).iter().any(|dir| Path::new(&path_c).parent() == Some(Path::new(&canon(dir))));
    let registered = list_with_dirty(repo, false)?.iter().skip(1).any(|wt| wt.path == path_c);
    if !registered && !managed {
        return Err(anyhow!("{path_c} is not a worktree of {repo_c}"));
    }
    // Relay never locks a worktree; someone did so to keep it. Refuse before purging anything.
    if is_locked(Path::new(&path_c)) {
        return Err(anyhow!("{path_c} is locked (git worktree unlock it first)"));
    }
    Ok((path_c, managed))
}

/// Pin a checkout's detached HEAD under `refs/relay/rescue/<checkout name>` when no branch, tag
/// or earlier rescue contains it, and say which ref. A clean checkout passes every dirty check,
/// yet commits made on a detached HEAD are reachable only from its HEAD and its reflog, and
/// removing the checkout deletes both. A ref never blocks the removal and loses nothing; any
/// `git` failure here refuses the removal instead. `git branch <name> <ref>` brings the work back.
fn rescue_detached(repo: &Path, path: &Path) -> Result<Option<String>> {
    // Without a `.git` of its own, git would answer for the primary checkout that holds it.
    if !path.join(".git").exists() || git(path, &["symbolic-ref", "-q", "HEAD"]).is_ok() {
        return Ok(None);
    }
    let Ok(head) = git(path, &["rev-parse", "--verify", "-q", "HEAD^{commit}"]) else { return Ok(None) };
    let head = head.trim();
    if !git(path, &["for-each-ref", "--count=1", "--contains", head, "refs/heads", "refs/tags", "refs/relay/rescue"])?.trim().is_empty() {
        return Ok(None);
    }
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let named = format!("refs/relay/rescue/{name}");
    let named = if !name.is_empty() && git(repo, &["check-ref-format", &named]).is_ok() { named } else { "refs/relay/rescue/detached".to_string() };
    // An empty old value creates the ref only if it is not there; a namesake from an earlier
    // rescue holds other commits, so this one takes the commit's id as a suffix.
    if git(repo, &["update-ref", &named, head, ""]).is_ok() {
        return Ok(Some(named));
    }
    let suffixed = format!("{named}-{}", &head[..head.len().min(12)]);
    git(repo, &["update-ref", &suffixed, head])?;
    Ok(Some(suffixed))
}

/// Whether `git worktree lock` holds this checkout: its admin slot carries a `locked` file.
fn is_locked(path: &Path) -> bool {
    let Ok(pointer) = read_regular_through_links(&path.join(".git")) else { return false };
    let Some(gitdir) = pointer.trim().strip_prefix("gitdir:") else { return false };
    let gitdir = PathBuf::from(gitdir.trim());
    let gitdir = if gitdir.is_absolute() { gitdir } else { path.join(gitdir) };
    gitdir.join("locked").exists()
}

/// Rename the branch checked out by one worktree. Used only before a session's first spawn.
pub fn rename_branch(worktree: &Path, branch: &str) -> Result<()> {
    if branch.trim().is_empty() || branch != branch.trim() {
        return Err(anyhow!("branch name cannot be empty or padded"));
    }
    git(worktree, &["check-ref-format", "--branch", branch])?;
    git(worktree, &["branch", "-m", branch])?;
    Ok(())
}

#[cfg(test)]
mod status_tests {
    use super::*;

    #[test]
    fn porcelain_preserves_renames_conflicts_and_unusual_paths() {
        let files = parse_status(b"R  new name\0old\nname\0?? untracked\nfile\0UU conflict\0 M changed\0").unwrap();
        assert_eq!(files[0].worktree, "M");
        assert_eq!(files[1].index, "U");
        assert_eq!(files[1].worktree, "U");
        assert_eq!(files[2].renamed_from.as_deref(), Some("old\nname"));
        assert_eq!(files[3].path, "untracked\nfile");
        assert_eq!(files[3].index, "");
        assert_eq!(files[3].worktree, "?");
    }

    #[test]
    fn repeated_status_does_not_rerun_filters_for_unchanged_assets() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        git(root, &["init", "-q", "-b", "main"]).unwrap();
        git(root, &["config", "user.name", "Fixture"]).unwrap();
        git(root, &["config", "user.email", "fixture@example.invalid"]).unwrap();
        std::fs::write(root.join(".gitattributes"), "*.bin filter=fixture\n").unwrap();
        let filter = root.join(".git/filter.sh");
        std::fs::write(&filter, "#!/bin/sh\necho run >> .git/filter-calls\ncat\n").unwrap();
        git(root, &["config", "filter.fixture.clean", &format!("sh {}", filter.display())]).unwrap();
        let asset = root.join("asset.bin");
        std::fs::write(&asset, vec![42u8; 1024 * 1024]).unwrap();
        let initial_time = std::time::SystemTime::now() - std::time::Duration::from_secs(30);
        std::fs::File::open(&asset).unwrap().set_modified(initial_time).unwrap();
        git(root, &["add", "."]).unwrap();
        git(root, &["commit", "-qm", "Asset"]).unwrap();
        // Simulate an editor touching an identical LFS-managed asset.
        std::fs::File::open(&asset).unwrap()
            .set_modified(initial_time + std::time::Duration::from_secs(5)).unwrap();
        std::fs::write(root.join(".git/filter-calls"), "").unwrap();
        // The commit is a few seconds old: git is done in this checkout (RA-131).
        std::fs::File::options().write(true).open(root.join(".git/index")).unwrap()
            .set_modified(initial_time + std::time::Duration::from_secs(10)).unwrap();
        assert!(status_files(root).unwrap().is_empty());
        let first = std::fs::read_to_string(root.join(".git/filter-calls")).unwrap();
        assert!(!first.is_empty(), "Fixture must exercise the clean filter");
        for _ in 0..5 { assert!(status_files(root).unwrap().is_empty()); }
        assert_eq!(std::fs::read_to_string(root.join(".git/filter-calls")).unwrap(), first,
            "Unchanged assets must use the refreshed stat cache");
        // The optimization must still detect real edits and preserve staged content.
        std::fs::write(&asset, "changed asset").unwrap();
        assert_eq!(status_files(root).unwrap()[0].worktree, "M");
        git(root, &["add", "asset.bin"]).unwrap();
        assert_eq!(status_files(root).unwrap()[0].index, "M");
    }

    /// RA-205: a directory with nothing tracked in it is one entry when collapsed, every file
    /// otherwise, and a checkout holding only that directory is still dirty.
    #[test]
    fn untracked_directories_collapse_without_hiding_the_change() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        git(root, &["init", "-q", "-b", "main"]).unwrap();
        git(root, &["config", "user.name", "Fixture"]).unwrap();
        git(root, &["config", "user.email", "fixture@example.invalid"]).unwrap();
        std::fs::write(root.join("a"), "a").unwrap();
        git(root, &["add", "a"]).unwrap();
        git(root, &["commit", "-qm", "a"]).unwrap();
        assert!(!is_dirty(&gix::open(root).unwrap()));
        std::fs::create_dir_all(root.join("vendor/pkg")).unwrap();
        for n in 0..20 { std::fs::write(root.join(format!("vendor/pkg/{n}")), "x").unwrap(); }
        let collapsed = status_files_with(root, Untracked::Directories).unwrap();
        assert_eq!(collapsed.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["vendor/"]);
        assert_eq!(collapsed[0].worktree, "?");
        assert_eq!(status_files(root).unwrap().len(), 20);
        assert!(is_dirty(&gix::open(root).unwrap()));
    }

    /// RA-131: only the first scan of a quiet checkout takes the index lock; the scans after it,
    /// or any while git is at work there, leave it to whoever runs git next.
    #[test]
    fn background_status_takes_the_index_lock_rarely() {
        let directory = tempfile::tempdir().unwrap();
        let root = &std::fs::canonicalize(directory.path()).unwrap();
        git(root, &["init", "-q", "-b", "main"]).unwrap();
        git(root, &["config", "user.name", "Fixture"]).unwrap();
        git(root, &["config", "user.email", "fixture@example.invalid"]).unwrap();
        std::fs::write(root.join("a"), "a").unwrap();
        git(root, &["add", "a"]).unwrap();
        git(root, &["commit", "-qm", "a"]).unwrap();
        let linked = root.join("linked");
        git(root, &["worktree", "add", "-q", "-b", "side", linked.to_str().unwrap()]).unwrap();
        let index = root.join(".git/index");
        let quiet = std::time::SystemTime::now() - std::time::Duration::from_secs(60);
        std::fs::File::options().write(true).open(&index).unwrap().set_modified(quiet).unwrap();
        // Someone holds the lock: never.
        std::fs::write(root.join(".git/index.lock"), "").unwrap();
        assert!(!may_lock_index(root));
        std::fs::remove_file(root.join(".git/index.lock")).unwrap();
        assert!(may_lock_index(root), "a quiet checkout gets its stat data saved");
        assert!(!may_lock_index(root), "but not on every scan");
        // A linked worktree's index is its own, and an index written just now means git is busy.
        assert_eq!(index_file(&linked).unwrap(), root.join(".git/worktrees/linked/index"));
        assert!(!may_lock_index(&linked));
    }

    /// RA-346: a copy of a linked worktree names a real slot, but the slot names the original.
    /// One moved by hand is told to run `git worktree repair` rather than refused as a stranger.
    #[test]
    fn only_the_checkout_a_slot_points_back_at_is_a_worktree() {
        let directory = tempfile::tempdir().unwrap();
        let root = &std::fs::canonicalize(directory.path()).unwrap().join("repo");
        std::fs::create_dir_all(root).unwrap();
        git(root, &["init", "-q", "-b", "main"]).unwrap();
        git(root, &["-c", "user.name=F", "-c", "user.email=f@f", "commit", "-q", "--allow-empty", "-m", "a"]).unwrap();
        let linked = root.parent().unwrap().join("linked");
        git(root, &["worktree", "add", "-q", "-b", "side", linked.to_str().unwrap()]).unwrap();
        assert!(contains(root, root).unwrap());
        assert!(contains(root, &linked).unwrap());
        let copy = root.parent().unwrap().join("copy");
        std::fs::create_dir_all(&copy).unwrap();
        std::fs::copy(linked.join(".git"), copy.join(".git")).unwrap();
        assert!(!contains(root, &copy).unwrap(), "a copy borrowing the original's slot passed");
        let moved = root.parent().unwrap().join("moved");
        std::fs::rename(&linked, &moved).unwrap();
        let error = contains(root, &moved).unwrap_err().to_string();
        assert!(error.contains("git worktree repair"), "{error}");
        git(&moved, &["worktree", "repair"]).unwrap();
        assert!(contains(root, &moved).unwrap());
    }

    /// A FIFO planted at a checkout's `.git`, at its slot's `gitdir` or `commondir`, or at the
    /// shared `info/exclude` is refused at once: callers read them under the exclude lock,
    /// `hooks::writes()` or the store mutex, and a plain read waits for a writer forever. A
    /// `.git` pointer reached through a link is still read, as git reads it.
    #[test]
    fn planted_fifos_in_git_pointers_and_exclude_do_not_block() {
        use std::os::unix::fs::FileTypeExt as _;
        fn mkfifo(path: &Path) {
            let name = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
            assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o644) }, 0, "mkfifo {}", path.display());
        }
        let directory = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(directory.path()).unwrap().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        git(&root, &["init", "-q", "-b", "main"]).unwrap();
        git(&root, &["-c", "user.name=F", "-c", "user.email=f@f", "commit", "-q", "--allow-empty", "-m", "a"]).unwrap();
        let linked = root.parent().unwrap().join("linked");
        git(&root, &["worktree", "add", "-q", "-b", "side", linked.to_str().unwrap()]).unwrap();
        let pointer = root.parent().unwrap().join("pointer");
        std::fs::rename(linked.join(".git"), &pointer).unwrap();
        std::os::unix::fs::symlink(&pointer, linked.join(".git")).unwrap();
        assert!(contains(&root, &linked).unwrap(), "a linked pointer is no longer read");
        let planted = root.parent().unwrap().join("planted");
        std::fs::create_dir_all(&planted).unwrap();
        mkfifo(&planted.join(".git"));
        let slot = root.join(".git/worktrees/linked");
        std::fs::remove_file(slot.join("commondir")).unwrap();
        mkfifo(&slot.join("commondir"));
        let exclude = root.join(".git/info/exclude");
        let _ = std::fs::remove_file(&exclude);
        std::fs::create_dir_all(exclude.parent().unwrap()).unwrap();
        mkfifo(&exclude);

        let (done, finished) = std::sync::mpsc::channel();
        let (root2, linked2, planted2) = (root.clone(), linked.clone(), planted.clone());
        std::thread::spawn(move || {
            let planted = (contains(&root2, &planted2).ok(), checkout_git_dir(&planted2), is_locked(&planted2));
            let common = common_dir(&linked2).ok();
            let excluded = exclude_paths(&root2, &[".relay/"]).is_err();
            std::fs::remove_file(root2.join(".git/worktrees/linked/gitdir")).unwrap();
            mkfifo(&root2.join(".git/worktrees/linked/gitdir"));
            let back = contains(&root2, &linked2).ok();
            done.send((planted, common, excluded, back)).unwrap();
        });
        let (planted, common, excluded, back) = finished.recv_timeout(std::time::Duration::from_secs(10))
            .expect("a read blocked on a planted FIFO");
        assert_eq!(planted, (Some(false), None, false), "a FIFO at .git is no worktree");
        assert_eq!(common, Some(slot), "an unreadable commondir leaves the slot itself");
        assert!(excluded, "a FIFO at info/exclude was written over");
        assert!(std::fs::symlink_metadata(&exclude).unwrap().file_type().is_fifo());
        assert_eq!(back, Some(false), "a slot whose gitdir is a FIFO names no checkout");
    }

    /// RA-347: a build dir reached through a symlinked parent is someone else's: neither purged
    /// nor counted. A real one is both, and the one-pass usage agrees with the purge.
    #[test]
    fn build_dirs_behind_a_symlink_are_left_alone() {
        let directory = tempfile::tempdir().unwrap();
        let wt = directory.path().join("wt");
        let shared = directory.path().join("shared/node_modules");
        std::fs::create_dir_all(shared.join(".cache")).unwrap();
        std::fs::write(shared.join(".cache/blob"), b"shared").unwrap();
        std::fs::create_dir_all(wt.join("target/debug")).unwrap();
        std::fs::write(wt.join("target/debug/app"), b"build").unwrap();
        std::fs::write(wt.join("main.rs"), b"src").unwrap();
        std::os::unix::fs::symlink(&shared, wt.join("node_modules")).unwrap();
        assert_eq!(build_size(&wt), 5);
        assert_eq!(disk_usage(&wt), (8, 5));
        assert_eq!(purge_build(&wt), 5);
        assert!(shared.join(".cache/blob").exists(), "the shared install's cache was purged");
        assert!(!wt.join("target").exists() && wt.join("main.rs").exists());
    }

    /// The shared-stack walk must total exactly what a single-threaded walk would, terminate
    /// with every worker parked on the condvar at some point, and ignore symlinks.
    #[test]
    fn a_parallel_disk_walk_totals_the_same_bytes_as_a_serial_one() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path();
        let mut expected = 0u64;
        // Deep on one side, wide on the other: both starve a walk that hands out one subtree.
        let mut deep = root.to_path_buf();
        for level in 0..40 {
            deep = deep.join(format!("level{level}"));
            std::fs::create_dir_all(&deep).unwrap();
            let body = vec![b'd'; level + 1];
            std::fs::write(deep.join("file.bin"), &body).unwrap();
            expected += body.len() as u64;
        }
        for branch in 0..50 {
            let wide = root.join(format!("wide{branch}"));
            std::fs::create_dir_all(&wide).unwrap();
            for file in 0..10 {
                let body = vec![b'w'; branch + file + 1];
                std::fs::write(wide.join(format!("f{file}")), &body).unwrap();
                expected += body.len() as u64;
            }
        }
        std::fs::write(root.join("top"), b"top").unwrap();
        expected += 3;
        #[cfg(unix)]
        {
            // A symlink to a real file must not be counted, and a loop must not hang the walk.
            std::os::unix::fs::symlink(root.join("top"), root.join("link")).unwrap();
            std::os::unix::fs::symlink(root, root.join("loop")).unwrap();
        }
        assert_eq!(dir_size(root), expected);
        // Idempotent, and empty or missing roots are zero.
        assert_eq!(dir_size(root), expected);
        assert_eq!(dir_size(&root.join("does-not-exist")), 0);
        let empty = root.join("empty");
        std::fs::create_dir(&empty).unwrap();
        assert_eq!(dir_size(&empty), 0);
    }
}
