//! Event-driven worktree watchers. A short trailing debounce folds editor saves and git's
//! lock/rename sequence into one refresh event without introducing an idle polling loop.

use crate::engine::{Engine, Unlocked};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long the watcher waits after the first event of a burst before it emits one
/// `file.changed`. Tests derive their "nothing arrived" windows from it.
pub const DEBOUNCE: Duration = Duration::from_millis(125);

// A status refresh can rewrite only index stat fields. Compare the staged tree
// rather than index bytes so this cache maintenance does not trigger another scan.
fn index_signature(root: &Path) -> Option<u64> {
    use std::hash::{Hash, Hasher};
    let repo = gix::open(root).ok()?;
    let index = repo.index().ok()?;
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    for entry in index.entries() {
        entry.path(&index).hash(&mut hash);
        entry.id.hash(&mut hash);
        entry.mode.bits().hash(&mut hash);
        entry.flags.bits().hash(&mut hash);
    }
    Some(hash.finish())
}

/// Most worktrees watched at once. Each is an inotify instance and a thread of its own; the
/// one requested least recently goes first, and a worktree that is deleted goes at once (RA-130).
const MAX_ROOTS: usize = 32;
/// Most directories one worktree registers, so one enormous tree cannot spend the user's whole
/// inotify watch limit.
const MAX_DIRS: usize = 16 * 1024;
/// A worktree whose watcher could not be set up is not walked again before this (RA-129).
const RETRY_AFTER: Duration = Duration::from_secs(60);

/// One watched worktree in [`Engine::watchers`]. `watcher` is `None` after a failed setup;
/// `used` is then when it failed.
pub(crate) struct Root {
    watcher: Option<Arc<Mutex<RecommendedWatcher>>>,
    used: Instant,
}

/// Registration may enumerate a large worktree. Start it once the current (unlocked) handler
/// returns, on a thread of its own, so first-time registration cannot block PTY or UI requests.
pub(crate) fn ensure_after_commit(
    ctx: &mut Unlocked<'_>,
    root: PathBuf,
    project_id: relay_bus::types::Id,
) {
    let root = std::fs::canonicalize(&root).unwrap_or(root);
    ctx.after_commit(move |engine| {
        let key = root.display().to_string();
        if let Some(known) = engine.watchers.lock().unwrap().get_mut(&key) {
            if known.watcher.is_some() {
                known.used = Instant::now();
                return;
            }
            if known.used.elapsed() < RETRY_AFTER {
                return;
            }
        }
        if !engine
            .watcher_registrations
            .lock()
            .unwrap()
            .insert(key.clone())
        {
            return;
        }
        let thread_engine = engine.clone();
        if std::thread::Builder::new()
            .name("watch-register".into())
            .spawn(move || ensure(&thread_engine, &root, project_id))
            .is_err()
        {
            engine.watcher_registrations.lock().unwrap().remove(&key);
        }
    });
}

pub fn ensure(engine: &Engine, root: &Path, project_id: relay_bus::types::Id) {
    let key = std::fs::canonicalize(root)
        .unwrap_or_else(|_| root.to_path_buf())
        .display()
        .to_string();
    let Some(engine) = engine.arc() else { return };
    let weak = Arc::downgrade(&engine);
    let pending = Arc::new(AtomicBool::new(false));
    let pending_cb = pending.clone();
    let root = PathBuf::from(&key);
    let callback_root = root.clone();
    let callback_key = key.clone();
    let mut last_index = index_signature(&root);
    // A build touches thousands of files, and every one of them arrives here as a path to
    // compare. Build the path being compared against once, not once per event path. A linked
    // worktree's index and HEAD are in its slot under the primary's `.git/worktrees/`, outside
    // this root: that slot is watched too, or staging there never refreshed anything (RA-344).
    let git_dir = crate::worktree::checkout_git_dir(&root)
        .and_then(|dir| std::fs::canonicalize(dir).ok())
        .unwrap_or_else(|| root.join(".git"));
    let slot = (!git_dir.starts_with(&root)).then(|| git_dir.clone());
    let callback_slot = slot.clone();
    let index_path = git_dir.join("index");
    let head_path = git_dir.join("HEAD");
    // A commit, an amend or a `reset --soft` in a linked worktree leaves its index's entries and
    // its HEAD file alone (HEAD names the branch, whose ref is in the common dir): what moves in
    // the slot is the reflog, `logs/HEAD`, so that folder is watched too.
    let logs_path = git_dir.join("logs");
    let logs_head = logs_path.join("HEAD");
    let callback_logs = logs_path.clone();
    let watcher: notify::Result<RecommendedWatcher> =
        notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            let Ok(event) = event else { return };
            // Refreshing the tree and Git state reads these same watched files.
            // Access notifications must not turn one mutation into an idle refresh loop.
            if matches!(event.kind, notify::EventKind::Access(_)) && !event.need_rescan() {
                return;
            }
            // Directories are watched one by one, so a new one is added here — and a deleted
            // worktree lets its watcher go. Both on another thread: the watcher's own calls
            // wait on the thread running this callback.
            let gone = matches!(event.kind, notify::EventKind::Remove(_))
                && event.paths.iter().any(|path| path == &callback_root);
            let created: Vec<PathBuf> = match event.kind {
                notify::EventKind::Create(_) | notify::EventKind::Modify(notify::event::ModifyKind::Name(_)) => event
                    .paths
                    .iter()
                    .filter(|path| {
                        path.starts_with(&callback_root)
                            && std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir())
                            && watched_dir(&callback_root, path)
                    })
                    .cloned()
                    .collect(),
                _ => Vec::new(),
            };
            // The slot's `logs/` arrives with its first reflog entry.
            let new_logs = callback_slot.is_some()
                && matches!(event.kind, notify::EventKind::Create(_))
                && event.paths.iter().any(|path| path == &callback_logs);
            if gone || !created.is_empty() || new_logs {
                let (weak, key, root, logs) = (weak.clone(), callback_key.clone(), callback_root.clone(), callback_logs.clone());
                std::thread::spawn(move || {
                    let Some(engine) = weak.upgrade() else { return };
                    if gone {
                        engine.watchers.lock().unwrap().remove(&key);
                        return;
                    }
                    let watcher = engine.watchers.lock().unwrap().get(&key).and_then(|known| known.watcher.clone());
                    if let Some(watcher) = watcher {
                        let mut watcher = watcher.lock().unwrap();
                        for dir in created.iter().flat_map(|dir| watch_dirs(&root, dir)) {
                            let _ = watcher.watch(&dir, RecursiveMode::NonRecursive);
                        }
                        if new_logs {
                            let _ = watcher.watch(&logs, RecursiveMode::NonRecursive);
                        }
                    }
                });
            }
            if (!event.need_rescan()
                && event
                    .paths
                    .iter()
                    .all(|path| {
                        if path == &index_path {
                            let next = index_signature(&callback_root);
                            let unchanged = next.is_some() && next == last_index;
                            last_index = next;
                            unchanged
                        } else if callback_slot.as_ref().is_some_and(|slot| path.starts_with(slot)) {
                            path != &head_path && path != &logs_head
                        } else {
                            is_generated_path(&callback_root, path)
                        }
                    }))
                || pending_cb.swap(true, Ordering::SeqCst)
            {
                return;
            }
            let weak = weak.clone();
            let pending = pending_cb.clone();
            let root = callback_root.clone();
            std::thread::spawn(move || {
                std::thread::sleep(DEBOUNCE);
                pending.store(false, Ordering::SeqCst);
                if let Some(engine) = weak.upgrade() {
                    let payload =
                        json!({"project_id": project_id, "worktree": root.display().to_string()});
                    // A filesystem mutation makes both the tree and Git projection stale. One
                    // scoped event lets the Code workspace refresh both without a guaranteed
                    // duplicate pass. Explicit git.* mutations still emit git.changed.
                    engine.emit_system("file.changed", payload);
                }
            });
        });
    // Not `RecursiveMode::Recursive`: that walks every build tree and every session checkout
    // under `.relay/`, follows symlinks out of the worktree, and gives up — dropping what it
    // had registered — at the first directory the watch limit refuses (RA-129).
    let watcher = match watcher {
        Ok(mut watcher) => {
            let dirs = watch_dirs(&root, &root);
            let mut added = 0usize;
            for dir in &dirs {
                match watcher.watch(dir, RecursiveMode::NonRecursive) {
                    Ok(()) => added += 1,
                    Err(error) if matches!(error.kind, notify::ErrorKind::MaxFilesWatch) => {
                        tracing::warn!(root = %key, added, "inotify watch limit reached; the rest of this worktree is not watched");
                        break;
                    }
                    // Deleted since the walk.
                    Err(_) => {}
                }
            }
            if let Some(slot) = &slot {
                let _ = watcher.watch(slot, RecursiveMode::NonRecursive);
                let _ = watcher.watch(&logs_path, RecursiveMode::NonRecursive);
            }
            if dirs.len() >= MAX_DIRS {
                tracing::warn!(root = %key, limit = MAX_DIRS, "worktree has more directories than are watched");
            }
            (added > 0).then_some(watcher)
        }
        Err(error) => {
            tracing::warn!(root = %key, error = %error, "could not watch worktree");
            None
        }
    };
    let registered = watcher.is_some();
    {
        let mut watchers = engine.watchers.lock().unwrap();
        if watchers.get(&key).is_none_or(|known| known.watcher.is_none()) {
            // Room first: worktrees that no longer exist, then the least recently requested.
            watchers.retain(|path, _| Path::new(path).exists());
            while watchers.len() >= MAX_ROOTS {
                let Some(oldest) = watchers.iter().min_by_key(|(_, known)| known.used).map(|(path, _)| path.clone()) else { break };
                watchers.remove(&oldest);
            }
            watchers.insert(key.clone(), Root {
                watcher: watcher.map(|watcher| Arc::new(Mutex::new(watcher))),
                used: Instant::now(),
            });
        }
    }
    engine.watcher_registrations.lock().unwrap().remove(&key);
    if registered {
        engine.emit_system(
            "file.changed",
            json!({"project_id": project_id, "worktree": root.display().to_string()}),
        );
    }
}

/// The directories under `from` that `root`'s watcher covers: no build output, none of Relay's
/// own checkouts under `.relay/`, nothing reached through a symlink, and of `.git` only the
/// parts a refresh cares about. At most [`MAX_DIRS`].
fn watch_dirs(root: &Path, from: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![from.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if out.len() >= MAX_DIRS {
            break;
        }
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                // `file_type` does not follow symlinks: a link to a directory is not descended.
                if entry.file_type().is_ok_and(|kind| kind.is_dir()) && watched_dir(root, &entry.path()) {
                    stack.push(entry.path());
                }
            }
        }
        out.push(dir);
    }
    out
}

fn watched_dir(root: &Path, dir: &Path) -> bool {
    match dir.strip_prefix(root).ok().and_then(|relative| relative.strip_prefix(".git").ok()) {
        Some(git) => git.as_os_str().is_empty() || git.starts_with("refs"),
        None => !is_generated_path(root, dir),
    }
}

/// Build caches can change hundreds of times per second while Relay itself is compiling. They are
/// never useful refresh signals for the source tree and would otherwise keep the single bus busy.
pub(crate) fn is_generated_path(root: &Path, path: &Path) -> bool {
    let relative = path.strip_prefix(root).unwrap_or(path);
    // Status filters may write .git/lfs and objects; these writes are not source
    // edits and must not start another status scan. Keep real index/ref changes.
    if let Ok(git_path) = relative.strip_prefix(".git") {
        return !(git_path == Path::new("index")
            || git_path == Path::new("HEAD")
            || git_path == Path::new("packed-refs")
            || git_path == Path::new("config")
            || (git_path.starts_with("refs") && !git_path.to_string_lossy().ends_with(".lock")));
    }
    relative.components().any(|component| generated_name(&component.as_os_str().to_string_lossy()))
}

/// A directory name that is build output or a cache wherever it appears. Cheap and by name only:
/// right for filtering watcher events, but a tracked directory can carry one of these names too
/// (a committed `dist/`), so `file.tree` and `file.search` also ask the index (RA-345).
pub(crate) fn generated_name(value: &str) -> bool {
    matches!(
        value,
        ".relay"
            | "node_modules"
            | "target"
            | "build"
            | ".gradle"
            | ".svelte-kit"
            | ".next"
            | "dist"
            | "coverage"
            | "Intermediate"
            | "Saved"
            | "DerivedDataCache"
            | "Binaries"
    )
}

#[cfg(test)]
mod tests {
    use super::is_generated_path;
    use std::path::Path;

    #[tokio::test]
    async fn reading_the_tree_does_not_refresh_it_but_writing_does() {
        use std::time::Duration;
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("source.txt");
        std::fs::write(&file, "before").unwrap();
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["add", "."],
            vec![
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@relay.test",
                "commit",
                "-qm",
                "Fixture",
            ],
        ] {
            assert!(std::process::Command::new("git")
                .arg("-C")
                .arg(directory.path())
                .args(args)
                .status()
                .unwrap()
                .success());
        }
        let engine =
            crate::Engine::new(crate::Instance::Test, crate::Store::open_memory().unwrap());
        let mut events = engine.subscribe();
        super::ensure(&engine, directory.path(), 1);
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), events.recv())
                .await
                .unwrap()
                .unwrap()
                .ev,
            "file.changed"
        );
        for _ in 0..10 {
            std::fs::read(&file).unwrap();
            std::fs::read_dir(directory.path())
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert!(std::process::Command::new("git")
                .env("GIT_OPTIONAL_LOCKS", "0")
                .arg("-C")
                .arg(directory.path())
                .args(["status", "--porcelain"])
                .output()
                .unwrap()
                .status
                .success());
        }
        assert!(
            tokio::time::timeout(Duration::from_millis(400), events.recv())
                .await
                .is_err(),
            "Read-only refresh caused another refresh"
        );
        std::fs::write(&file, "after").unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), events.recv())
                .await
                .unwrap()
                .unwrap()
                .ev,
            "file.changed"
        );
    }

    #[test]
    fn only_the_source_tree_is_walked() {
        let fixture = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(fixture.path()).unwrap();
        let outside = tempfile::tempdir().unwrap();
        for dir in ["src/a", "target/debug/deep", "web/node_modules/pkg", ".relay/worktrees/s/src",
            ".git/objects/ab", ".git/refs/heads", ".git/logs/refs"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        std::fs::create_dir_all(outside.path().join("elsewhere")).unwrap();
        std::os::unix::fs::symlink(outside.path(), root.join("link")).unwrap();
        let dirs = super::watch_dirs(&root, &root);
        for wanted in ["", "src", "src/a", "web", ".git", ".git/refs", ".git/refs/heads"] {
            assert!(dirs.contains(&root.join(wanted)), "{wanted} is not watched: {dirs:?}");
        }
        assert_eq!(dirs.len(), 7, "{dirs:?}");
    }

    /// A new directory is watched as it appears, and a deleted worktree lets its watcher go.
    #[tokio::test]
    async fn new_directories_are_watched_and_a_deleted_root_is_dropped() {
        use std::time::Duration;
        let fixture = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(fixture.path()).unwrap().join("wt");
        std::fs::create_dir_all(&root).unwrap();
        let engine = crate::Engine::new(crate::Instance::Test, crate::Store::open_memory().unwrap());
        // Subscribed when called, so each one sees only what happens after it.
        let next = || {
            let mut events = engine.subscribe();
            async move { tokio::time::timeout(Duration::from_secs(2), events.recv()).await.map(|ev| ev.unwrap().ev) }
        };
        let first = next();
        super::ensure(&engine, &root, 1);
        assert_eq!(first.await.unwrap(), "file.changed");
        let created = next();
        std::fs::create_dir(root.join("fresh")).unwrap();
        assert_eq!(created.await.unwrap(), "file.changed");
        tokio::time::sleep(Duration::from_millis(300)).await;
        let written = next();
        std::fs::write(root.join("fresh/file.txt"), "x").unwrap();
        assert_eq!(written.await.unwrap(), "file.changed", "an edit in a new directory went unseen");

        let key = root.display().to_string();
        assert!(engine.watchers.lock().unwrap().get(&key).is_some_and(|known| known.watcher.is_some()));
        std::fs::remove_dir_all(&root).unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            while engine.watchers.lock().unwrap().contains_key(&key) {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("the deleted worktree's watcher is still held");
    }

    /// RA-344: a linked worktree's index lives in its slot under the primary's `.git`, outside
    /// the watched root. Staging there must still refresh it.
    #[tokio::test]
    async fn staging_in_a_linked_worktree_refreshes_it() {
        use std::time::Duration;
        let fixture = tempfile::tempdir().unwrap();
        let repo = std::fs::canonicalize(fixture.path()).unwrap().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let git = |dir: &Path, args: &[&str]| assert!(std::process::Command::new("git").arg("-C").arg(dir).args(args).status().unwrap().success());
        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["-c", "user.name=F", "-c", "user.email=f@f", "commit", "-q", "--allow-empty", "-m", "a"]);
        let linked = repo.parent().unwrap().join("linked");
        git(&repo, &["worktree", "add", "-q", "-b", "side", linked.to_str().unwrap()]);
        std::fs::write(linked.join("new.txt"), "x").unwrap();
        let engine = crate::Engine::new(crate::Instance::Test, crate::Store::open_memory().unwrap());
        let next = || {
            let mut events = engine.subscribe();
            async move { tokio::time::timeout(Duration::from_secs(2), events.recv()).await.map(|ev| ev.unwrap().ev) }
        };
        let first = next();
        super::ensure(&engine, &linked, 1);
        assert_eq!(first.await.unwrap(), "file.changed");
        tokio::time::sleep(Duration::from_millis(300)).await;
        let staged = next();
        git(&linked, &["add", "new.txt"]);
        assert_eq!(staged.await.expect("staging in a linked worktree went unseen"), "file.changed");
        // A commit leaves the index's entries and HEAD alone; its reflog line is what is seen.
        tokio::time::sleep(Duration::from_millis(300)).await;
        let committed = next();
        git(&linked, &["-c", "user.name=F", "-c", "user.email=f@f", "commit", "-q", "-m", "b"]);
        assert_eq!(committed.await.expect("a commit in a linked worktree went unseen"), "file.changed");
        tokio::time::sleep(Duration::from_millis(300)).await;
        let reset = next();
        git(&linked, &["reset", "-q", "--soft", "HEAD~1"]);
        assert_eq!(reset.await.expect("a soft reset in a linked worktree went unseen"), "file.changed");
    }

    #[test]
    fn generated_build_paths_do_not_refresh_the_code_workspace() {
        let root = Path::new("/repo");
        assert!(is_generated_path(root, Path::new("/repo/target/debug/app")));
        assert!(is_generated_path(
            root,
            Path::new("/repo/apps/web/node_modules/pkg/index.js")
        ));
        assert!(is_generated_path(
            root,
            Path::new("/repo/app/build/generated/source.kt")
        ));
        assert!(!is_generated_path(
            root,
            Path::new("/repo/apps/web/src/App.svelte")
        ));
        assert!(!is_generated_path(root, Path::new("/repo/.git/HEAD")));
        for path in [".git/lfs/tmp/asset", ".git/objects/ab/object", ".git/index.lock",
            "Saved/Logs/Unreal.log", "Intermediate/Build/file", "DerivedDataCache/cache", "Binaries/Linux/game"] {
            assert!(is_generated_path(root, &root.join(path)), "{path}");
        }
        for path in [".git/index", ".git/refs/heads/main", "Content/Level.umap"] {
            assert!(!is_generated_path(root, &root.join(path)), "{path}");
        }
    }
}
