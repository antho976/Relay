//! `relay serve` (and `relay serve --remote`, which adds the phone door in the same process) does
//! this: take the instance lock, become the subreaper for everything below it, open the store
//! exclusively, build the engine, open the socket door, run until quit. The desktop app is a
//! client of this socket; it does not host an engine.

use crate::engine::Engine;
use crate::paths::Instance;
use crate::socket::{BindError, InstanceLock, SocketServer};
use crate::store::Store;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;

/// A running engine with its socket door. Keep it alive; drop to stop.
pub struct Served {
    pub engine: Arc<Engine>,
    pub socket: SocketServer,
}

impl Served {
    /// Stop: no new requests, the socket door closed and its connections ended, then every
    /// child killed and marked restorable. The door goes first, so nothing a client sends can
    /// land after [`Engine::shutdown`] has drained the PTYs (RA-333).
    pub fn stop(self) {
        let Served { engine, socket } = self;
        engine.request_quit();
        drop(socket);
        engine.shutdown();
    }
}

/// Take the instance lock, open the store (exclusive), build the engine, bind the socket. Does
/// not block.
pub async fn start(instance: Instance, store_path: Option<PathBuf>) -> Result<Served, BindError> {
    // The lock first: a second engine for this instance is told so, before it opens a store
    // or runs recovery on one (RA-624).
    let lock = InstanceLock::take(instance)?;
    // Before any child exists: a session descendant that double-forks must land under the
    // engine, not init, or the socket door would take it for the user (RA-096, D165).
    let subreaper = crate::peer::become_subreaper();
    if !subreaper {
        tracing::warn!(error = %std::io::Error::last_os_error(), "could not become a child subreaper; a session's orphans can claim the user actor");
    }
    // The instance's folder holds the store and the databases beside it (threads, money,
    // arbiter, gym); older installs made it 0755. A store path chosen by the caller keeps its
    // folder as it is.
    if store_path.is_none() {
        crate::private::private_data_dir(&instance.data_dir());
    }
    let path = store_path.unwrap_or_else(|| instance.store_path());
    let store = Store::open(&path, true).map_err(BindError::Other)?;
    let engine = Engine::new(instance, store);
    if subreaper {
        crate::peer::spawn_orphan_reaper(&engine);
    }
    if let Err(e) = crate::recovery::run_with(&engine, crate::recovery::DirtyScan::Deferred) {
        tracing::warn!(error = %e, "crash recovery failed; continuing");
    }
    // Every checkout picks up the app-wide skill folders once at start, so a workspace that
    // already existed when a skill was installed does not wait for its next launch (D147).
    // One shot on a worker, not a timer: nothing here repeats.
    let skills = engine.clone();
    std::thread::Builder::new()
        .name("skill-materialize".into())
        .spawn(move || crate::skills::refresh_all(&skills))
        .ok();
    // Branches whose PR merged after their session closed (branch_cleanup).
    crate::branch_cleanup::spawn_sweeper(&engine);
    // Retention: soft-deleted rows past the undo window, old notifications and mail (purge).
    crate::purge::spawn_timer(&engine);
    // Arbiter's strategies wake on their bars (arbiter::spawn_runner).
    crate::arbiter::spawn_runner(&engine);
    let socket = lock.bind(engine.clone()).await?;
    Ok(Served { engine, socket })
}

/// `relay serve`: start, then block until `app.quit` or SIGINT/SIGTERM.
pub async fn serve(instance: Instance, store_path: Option<PathBuf>) -> Result<(), BindError> {
    let served = start(instance, store_path).await?;
    tracing::info!(instance = %instance, store = %served.engine.store.path().display(), "engine up");
    let quit = served.engine.clone();
    tokio::select! {
        _ = quit.wait_quit() => tracing::info!("app.quit received"),
        _ = tokio::signal::ctrl_c() => tracing::info!("SIGINT"),
        _ = sigterm() => tracing::info!("SIGTERM"),
    }
    served.stop();
    Ok(())
}

async fn sigterm() {
    match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
        Ok(mut s) => {
            s.recv().await;
        }
        Err(_) => std::future::pending::<()>().await,
    }
}
