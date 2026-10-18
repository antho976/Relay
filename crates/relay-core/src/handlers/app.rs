//! `app.*` (BUS.md §10.2) — the phase-1 subset: version, status, quit, reconcile.

use crate::engine::{Ctx, Engine};
use relay_bus::error::BusError;
use relay_bus::ops::app::*;
use rusqlite::Connection;
use serde_json::json;
use std::collections::HashSet;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

pub fn register(e: &mut Engine) {
    e.register::<Version>(|ctx, _| {
        Ok(VersionOut {
            version: env!("CARGO_PKG_VERSION").to_string(),
            instance: ctx.instance().as_str().to_string(),
            build: BuildInfo {
                profile: if cfg!(debug_assertions) { "debug" } else { "release" }.to_string(),
                git_sha: option_env!("RELAY_GIT_SHA").map(str::to_string),
                // Nothing sets RELAY_BUILT_AT in a plain `cargo build`, so fall back to the
                // running image's own mtime: /proc/self/exe resolves to the inode this process
                // was started from, even after cargo has replaced the file, so a stale engine
                // reports an older time than the binary on disk (RA-271).
                built_at: option_env!("RELAY_BUILT_AT").map(str::to_string).or_else(|| {
                    let modified = std::fs::metadata("/proc/self/exe").or_else(|_| std::env::current_exe().and_then(std::fs::metadata)).ok()?.modified().ok()?;
                    Some(jiff::Timestamp::try_from(modified).ok()?.to_string())
                }),
            },
        })
    });
    e.register::<Status>(|ctx, _| {
        let eng = ctx.engine();
        Ok(StatusOut {
            pid: std::process::id(),
            uptime_s: eng.uptime_s(),
            store_path: eng.store.path().display().to_string(),
            socket_path: eng.socket_path.lock().unwrap().clone().unwrap_or_default(),
            sessions_live: eng.live_pty_count() as i64,
            providers: crate::providers::list(ctx.tx())?,
            gaming: {
                let g = crate::gaming::status();
                relay_bus::ops::app::GamingStatus { supported: g.supported, on: g.on, auto: g.auto, threads: g.threads, active: g.active, game: g.game }
            },
        })
    });
    e.register::<Quit>(|ctx, p| {
        let live = ctx.engine().live_pty_count() as i64;
        if live > 0 && !p.force.unwrap_or(false) {
            return Err(BusError::conflict("app.sessions_live", format!("{live} sessions are live; pass force to quit anyway")));
        }
        ctx.after_commit(|eng| eng.request_quit());
        Ok(relay_bus::Empty {})
    });
    e.register::<RecoveryLast>(|ctx, _| crate::recovery::last(ctx.tx()).map_err(crate::engine::internal));
    // The copy is the whole database: it runs before the transaction, taking the store lock one
    // step at a time (`Store::backup`), so requests keep flowing while it runs.
    e.register_staged::<BackupNow, BackupOut>(
        |ctx, _| {
            let path = ctx.engine().store.backup("manual").map_err(crate::engine::internal)?;
            let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            Ok(BackupOut { path: path.display().to_string(), bytes })
        },
        |_ctx, _, out| Ok(out),
    );
    e.register::<BackupList>(|ctx, _| {
        let backups = ctx.engine().store.list_backups().map_err(crate::engine::internal)?
            .into_iter()
            .map(|b| BackupInfo { path: b.path.display().to_string(), bytes: b.bytes, created_at: b.created_at, reason: b.reason })
            .collect();
        Ok(BackupListOut { backups })
    });
    // Reading the v3 store and copying its attachments is file work of any size: staged, with
    // the store unlocked; the transaction only writes the rows and links the copies (RA-382).
    e.register_staged::<ImportV3, _>(super::import_v3::prepare, super::import_v3::import);
    e.register::<FirstRunState>(|ctx, _| {
        let workspaces: i64 = ctx.tx().query_row("SELECT COUNT(*) FROM workspaces", [], |row| row.get(0)).map_err(crate::engine::internal)?;
        let providers: i64 = ctx.tx().query_row("SELECT COUNT(*) FROM provider_cache", [], |row| row.get(0)).map_err(crate::engine::internal)?;
        let projects: i64 = ctx.tx().query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0)).map_err(crate::engine::internal)?;
        let imports: i64 = ctx.tx().query_row("SELECT COUNT(*) FROM meta WHERE key LIKE 'import_v3:%'", [], |row| row.get(0)).map_err(crate::engine::internal)?;
        let mut steps = BTreeMap::new();
        steps.insert("workspace".into(), if workspaces > 0 { "done" } else { "todo" }.into());
        steps.insert("providers".into(), if providers >= 2 { "done" } else { "todo" }.into());
        steps.insert("project".into(), if projects > 0 { "done" } else { "todo" }.into());
        steps.insert("import".into(), if imports > 0 { "done" } else if projects > 0 { "skipped" } else { "todo" }.into());
        Ok(FirstRunOut { needed: workspaces == 0 || projects == 0, steps })
    });
    e.register::<ResourcesGet>(|ctx: &mut Ctx, _| {
        // Answer from the disk cache so the walk never runs under the store mutex (D144), then
        // re-measure on a worker once the transaction is gone. The fresh numbers arrive as the
        // same `resource.sample` event the panel already listens to.
        let snapshot = resources(ctx.tx(), ctx.engine())?;
        ctx.after_commit(|engine| {
            std::thread::Builder::new()
                .name("resource-disk".into())
                .spawn(move || {
                    crate::background_priority();
                    refresh_disk_cache(&engine)
                })
                .ok();
        });
        Ok(snapshot)
    });
    e.register::<ResourcesWatch>(|ctx: &mut Ctx, p| {
        let on = {
            let mut clients = ctx.engine().resource_watch_clients.lock().unwrap();
            *clients = if p.on { *clients + 1 } else { clients.saturating_sub(1) };
            let on = *clients > 0;
            let changed = ctx.engine().resource_watch.swap(on, Ordering::SeqCst) != on;
            changed && on
        };
        if on {
            let epoch = ctx.engine().resource_watch_epoch.fetch_add(1, Ordering::SeqCst) + 1;
            ctx.after_commit(move |engine| {
                let Ok(handle) = tokio::runtime::Handle::try_current() else { return };
                handle.spawn(async move {
                    let mut tick: u32 = 0;
                    while engine.resource_watch.load(Ordering::SeqCst) && engine.resource_watch_epoch.load(Ordering::SeqCst) == epoch {
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        if !engine.resource_watch.load(Ordering::SeqCst) || engine.resource_watch_epoch.load(Ordering::SeqCst) != epoch { break; }
                        tick = tick.wrapping_add(1);
                        // The store mutex and the /proc reads on a blocking worker: waiting on the lock here
                        // would park a runtime thread the socket door needs (RA-341).
                        let worker = engine.clone();
                        let _ = tokio::task::spawn_blocking(move || { watch_tick(&worker, tick); }).await;
                    }
                });
            });
        }
        Ok(relay_bus::Empty {})
    });
    e.register::<LogTail>(|_ctx, _p| {
        // No `log` stream exists yet: the engine's tracing output goes to stderr only. Saying so
        // beats a success that attaches nothing and leaves the caller waiting for records.
        Err(BusError::not_implemented("app.log.tail", 9))
    });
    e.register::<Reconcile>(|ctx, _| {
        // The trust-but-verify pass, on demand: retention, then the disk closed sessions left
        // behind (`disk_sweep`). The same pass also runs on the engine's own timer
        // (`purge::spawn_timer`).
        let store_dir = ctx.engine().store.path().parent().map(Path::to_path_buf).unwrap_or_default();
        let purged = crate::purge::run(ctx.tx(), &store_dir).map_err(crate::engine::internal)?;
        if !purged.actions.is_empty() {
            ctx.emit("notify.changed", json!({"purged": true}));
        }
        let paths = purged.paths;
        ctx.after_commit(move |engine| {
            std::thread::Builder::new()
                .name("purge-files".into())
                .spawn(move || {
                    crate::background_priority();
                    crate::purge::remove_paths(&paths);
                    crate::disk_sweep::run(&engine);
                })
                .ok();
        });
        Ok(ReconcileOut { actions: purged.actions })
    });
}

/// The resource watch re-measures disk on every this-many ticks (30 s at one tick per 2 s).
const DISK_EVERY: u32 = 15;

/// One resource-watch tick, without its timer. CPU and RSS are two /proc reads, so most ticks
/// publish a `resource.sample` with them and the cached disk numbers. Disk is a tree walk, so
/// every [`DISK_EVERY`]th tick re-measures it instead, on a thread of its own rather than a
/// pool one, so the walk can run niced without leaving a pool thread demoted for whatever runs
/// there next; that thread publishes its own sample. Returns it when one was started.
fn watch_tick(engine: &Arc<Engine>, tick: u32) -> Option<std::thread::JoinHandle<()>> {
    if tick.is_multiple_of(DISK_EVERY) {
        let engine = engine.clone();
        return std::thread::Builder::new()
            .name("resource-disk".into())
            .spawn(move || {
                crate::background_priority();
                refresh_disk_cache(&engine)
            })
            .ok();
    }
    let rows = {
        let conn = engine.store.lock();
        resource_rows(&conn).ok()
    };
    let value = rows.and_then(|rows| resource_snapshot(rows, engine, false).ok()).and_then(|value| serde_json::to_value(value).ok());
    if let Some(value) = value { engine.emit_system("resource.sample", value); }
    None
}

/// A snapshot from the disk cache. Never walks the filesystem, so it is safe to call from inside
/// a request transaction (D144). Use [`refresh_disk_cache`] off the mutex to keep the cache warm.
pub(crate) fn resources(conn: &Connection, engine: &Engine) -> Result<ResourcesOut, BusError> {
    resource_snapshot(resource_rows(conn)?, engine, false)
}

/// Re-measure every live worktree and publish the result. Walks the filesystem — call this from a
/// worker thread only, never with the store mutex held.
pub(crate) fn refresh_disk_cache(engine: &Engine) {
    // UI refreshes and resource-watch ticks can arrive together. At most one scan
    // runs, with a cooldown after completion rather than a pile-up of tree walks.
    {
        let mut refresh = engine.resource_disk_refresh.lock().unwrap();
        if refresh.0 || refresh.1.is_some_and(|at| at.elapsed() < std::time::Duration::from_secs(30)) {
            return;
        }
        refresh.0 = true;
    }
    struct Refresh<'a>(&'a Engine);
    impl Drop for Refresh<'_> {
        fn drop(&mut self) {
            *self.0.resource_disk_refresh.lock().unwrap() = (false, Some(Instant::now()));
        }
    }
    let _refresh = Refresh(engine);
    let rows = {
        let conn = engine.store.lock();
        resource_rows(&conn).ok()
    };
    let Some(rows) = rows else { return };
    let Ok(snapshot) = resource_snapshot(rows, engine, true) else { return };
    if let Ok(value) = serde_json::to_value(snapshot) {
        engine.emit_system("resource.sample", value);
    }
}

type ResourceRow = (String, Option<i64>, String);

fn resource_rows(conn: &Connection) -> Result<Vec<ResourceRow>, BusError> {
    let mut stmt = conn.prepare_cached(
        "SELECT name,pid,worktree FROM sessions WHERE state!='closed' ORDER BY id",
    ).map_err(crate::engine::internal)?;
    let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?, row.get::<_, String>(2)?)))
        .map_err(crate::engine::internal)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(crate::engine::internal)?;
    Ok(rows)
}

fn resource_snapshot(rows: Vec<ResourceRow>, engine: &Engine, refresh_disk: bool) -> Result<ResourcesOut, BusError> {
    let mut panes = Vec::new();
    let mut worktree_paths = HashSet::new();
    let mut total_rss_mb = 0.0;
    for (session, pid, worktree) in rows {
        if !worktree.is_empty() { worktree_paths.insert(worktree); }
        let rss_mb = pid.and_then(proc_rss_mb).unwrap_or(0.0);
        let cpu_pct = pid.and_then(|pid| proc_ticks(pid).map(|ticks| cpu_pct(engine, pid, ticks))).unwrap_or(0.0);
        total_rss_mb += rss_mb;
        panes.push(PaneResource { session, pid, rss_mb, cpu_pct });
    }
    // A cold cache is still a cache-only read. Missing entries arrive from the
    // background scan, never from the request holding the global store mutex.
    let missing = if refresh_disk { worktree_paths.iter().cloned().collect::<Vec<_>>() } else { Vec::new() };
    if !missing.is_empty() {
        let measured = missing.into_iter().map(|path| {
            let (disk, build) = crate::worktree::disk_usage(Path::new(&path));
            (path, (bytes_mb(disk), (build > 0).then(|| bytes_mb(build))))
        }).collect::<Vec<_>>();
        let mut cache = engine.resource_disk.lock().unwrap();
        for (path, value) in measured { cache.insert(path, value); }
    }
    let mut worktrees = {
        let mut cache = engine.resource_disk.lock().unwrap();
        cache.retain(|path, _| worktree_paths.contains(path));
        worktree_paths.into_iter().filter_map(|path| cache.get(&path).map(|(disk_mb, build_mb)| WorktreeDisk { path, disk_mb: *disk_mb, build_mb: *build_mb })).collect::<Vec<_>>()
    };
    worktrees.sort_by(|a, b| a.path.cmp(&b.path));
    let store_mb = std::fs::metadata(engine.store.path()).map(|m| bytes_mb(m.len())).unwrap_or(0.0);
    let relay_pid = i64::from(std::process::id());
    let relay = RelayResource {
        pid: relay_pid,
        rss_mb: proc_rss_mb(relay_pid).unwrap_or(0.0),
        cpu_pct: proc_ticks(relay_pid).map(|ticks| cpu_pct(engine, relay_pid, ticks)).unwrap_or(0.0),
    };
    // Only the pids sampled now keep a baseline: a pid that is gone, and later reused, must
    // not be measured against its predecessor's ticks.
    let sampled: HashSet<i64> = panes.iter().filter_map(|pane| pane.pid).chain([relay_pid]).collect();
    engine.resource_cpu.lock().unwrap().retain(|pid, _| sampled.contains(&pid.abs()));
    Ok(ResourcesOut { relay, panes, worktrees, store_mb, total_rss_mb })
}

fn bytes_mb(bytes: u64) -> f64 { bytes as f64 / (1024.0 * 1024.0) }

fn proc_rss_mb(pid: i64) -> Option<f64> {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let kb = status.lines().find_map(|line| line.strip_prefix("VmRSS:")?.split_whitespace().next()?.parse::<f64>().ok())?;
    Some(kb / 1024.0)
}

fn proc_ticks(pid: i64) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let (_, rest) = stat.rsplit_once(") ")?;
    let fields = rest.split_whitespace().collect::<Vec<_>>();
    Some(fields.get(11)?.parse::<u64>().ok()? + fields.get(12)?.parse::<u64>().ok()?)
}

/// The shortest window a CPU reading is taken over. At CLK_TCK=100 a window of a millisecond
/// or two holds zero ticks or one, so a busy pane would read 0% or 1000% (RA-348).
const CPU_MIN_WINDOW: std::time::Duration = std::time::Duration::from_millis(500);

/// CPU% of `pid` since an earlier sample. Every reader shares the baselines — the watch loop,
/// `app.resources.get`, its disk refresh, `dashboard.get` — so `pid` holds the latest one and
/// `-pid` (pids are positive) the one before it. A reader that comes too soon after the latest
/// measures from the earlier one instead and leaves both in place.
fn cpu_pct(engine: &Engine, pid: i64, ticks: u64) -> f64 {
    let now = Instant::now();
    let mut samples = engine.resource_cpu.lock().unwrap();
    let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) }.max(1) as f64;
    let pct = |(prior, at): (u64, Instant)| {
        (ticks.saturating_sub(prior) as f64 / hz / now.duration_since(at).as_secs_f64() * 100.0).max(0.0)
    };
    match samples.get(&pid).copied() {
        Some(latest) if now.duration_since(latest.1) < CPU_MIN_WINDOW => {
            samples.get(&-pid).copied().map(pct).unwrap_or(0.0)
        }
        latest => {
            if let Some(latest) = latest { samples.insert(-pid, latest); }
            samples.insert(pid, (ticks, now));
            latest.map(pct).unwrap_or(0.0)
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cold_resource_snapshot_leaves_disk_work_to_the_background() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("asset"), b"asset").unwrap();
        let engine = Engine::new(crate::Instance::Test, crate::Store::open_memory().unwrap());
        let rows = vec![("fixture".into(), None, root.path().display().to_string())];
        let snapshot = resource_snapshot(rows.clone(), &engine, false).unwrap();
        assert_eq!(snapshot.panes.len(), 1);
        assert!(snapshot.worktrees.is_empty(), "cache-only snapshot walked the worktree");
        assert!(engine.resource_disk.lock().unwrap().is_empty());
        let measured = resource_snapshot(rows.clone(), &engine, true).unwrap();
        assert_eq!(measured.worktrees[0].disk_mb, bytes_mb(5));
        std::fs::write(root.path().join("asset"), b"asset changed").unwrap();
        let cached = resource_snapshot(rows, &engine, false).unwrap();
        assert_eq!(cached.worktrees[0].disk_mb, bytes_mb(5));
    }

    #[test]
    fn disk_refreshes_do_not_overlap_or_repeat_inside_the_cooldown() {
        let engine = Engine::new(crate::Instance::Test, crate::Store::open_memory().unwrap());
        *engine.resource_disk_refresh.lock().unwrap() = (true, None);
        refresh_disk_cache(&engine);
        assert_eq!(*engine.resource_disk_refresh.lock().unwrap(), (true, None));
        let completed = Instant::now();
        *engine.resource_disk_refresh.lock().unwrap() = (false, Some(completed));
        refresh_disk_cache(&engine);
        assert_eq!(*engine.resource_disk_refresh.lock().unwrap(), (false, Some(completed)));
    }

    /// RA-674: the watch loop's body. Ticks between disk refreshes sample from the cache and
    /// never walk a worktree; every 15th re-measures disk on its own thread, and later ticks
    /// carry what it found.
    #[test]
    fn every_fifteenth_watch_tick_refreshes_disk_and_the_rest_read_the_cache() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("asset"), b"asset").unwrap();
        let engine = Engine::new(crate::Instance::Test, crate::Store::open_memory().unwrap());
        engine.store.with_tx(|tx| {
            tx.execute("INSERT INTO workspaces(path,name,ord,created_at,updated_at) VALUES ('/w','w',0,'t','t')", [])?;
            tx.execute("INSERT INTO projects(workspace_id,path,name,base_branch,ord,created_at,updated_at) VALUES (1,'/w/p','p','main',0,'t','t')", [])?;
            tx.execute("INSERT INTO sessions(name,project_id,provider,worktree,state,created_at,updated_at) VALUES ('fixture',1,'claude',?1,'running','t','t')",
                [root.path().display().to_string()])?;
            Ok(())
        }).unwrap();
        let mut events = engine.subscribe();
        let mut sample = || {
            let event = events.try_recv().expect("tick published no sample");
            assert_eq!(event.ev, "resource.sample");
            event.payload
        };
        for tick in 1..DISK_EVERY {
            assert!(watch_tick(&engine, tick).is_none(), "tick {tick} started a disk walk");
            let value = sample();
            assert_eq!(value["panes"][0]["session"], "fixture");
            assert_eq!(value["worktrees"], json!([]), "tick {tick} walked the worktree");
        }
        assert_eq!(*engine.resource_disk_refresh.lock().unwrap(), (false, None));
        watch_tick(&engine, DISK_EVERY).expect("tick 15 started no disk walk").join().unwrap();
        assert_eq!(sample()["worktrees"][0]["disk_mb"], bytes_mb(5));
        assert!(engine.resource_disk_refresh.lock().unwrap().1.is_some());
        std::fs::write(root.path().join("asset"), b"asset changed").unwrap();
        assert!(watch_tick(&engine, DISK_EVERY + 1).is_none());
        assert_eq!(sample()["worktrees"][0]["disk_mb"], bytes_mb(5), "a sampling tick re-measured disk");
    }

    #[test]
    fn disk_usage_counts_total_and_build_bytes_in_one_walk() {
        // The same build dirs a purge deletes (RA-629): top-level `target/`, not `src/build/`.
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("src")).unwrap();
        std::fs::create_dir_all(root.path().join("target/debug")).unwrap();
        std::fs::write(root.path().join("src/main.rs"), b"source").unwrap();
        std::fs::write(root.path().join("target/debug/app"), b"build-output").unwrap();
        std::fs::create_dir_all(root.path().join("src/build")).unwrap();
        std::fs::write(root.path().join("src/build/gen.rs"), b"tracked").unwrap();
        assert_eq!(crate::worktree::disk_usage(root.path()), (25, 12));
    }

    #[test]
    fn a_cpu_reading_right_after_another_measures_from_the_earlier_baseline() {
        let engine = Engine::new(crate::Instance::Test, crate::Store::open_memory().unwrap());
        let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) }.max(1) as u64;
        let start = Instant::now() - std::time::Duration::from_secs(2);
        engine.resource_cpu.lock().unwrap().insert(7, (0, start));
        // Two seconds at one full core: 100%.
        let first = cpu_pct(&engine, 7, 2 * hz);
        assert!((first - 100.0).abs() < 5.0, "{first}");
        // Milliseconds later, one more tick: the reading still covers about two seconds, never
        // the millisecond window that would make it 0% or 1000%.
        let again = cpu_pct(&engine, 7, 2 * hz + 1);
        assert!((90.0..110.0).contains(&again), "{again}");
        assert_eq!(engine.resource_cpu.lock().unwrap()[&7].0, 2 * hz, "the short read moved the baseline");
        // A pid no longer among the sessions loses its baselines.
        resource_snapshot(Vec::new(), &engine, false).unwrap();
        assert!(!engine.resource_cpu.lock().unwrap().keys().any(|pid| pid.abs() == 7));
    }
}
