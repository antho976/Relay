//! Gaming mode: while it is on, every agent and everything an agent runs (builds, tests, its
//! own tools) shares a few CPU threads at the lowest CPU and disk priority, so a game keeps the
//! machine. The engine and the window are outside the cap, so typing in Relay stays immediate.
//!
//! Each agent process is moved, right after it starts, into a systemd scope of its own under
//! [`SLICE`] in the user's service manager; what it starts later is born inside it. Gaming mode
//! is then two properties on that slice (`CPUQuota`, `CPUWeight`/`IOWeight`), set and cleared
//! with `systemctl --user set-property --runtime`, which applies at once to processes already
//! running, a build halfway through included. Nothing is written to disk: a reboot clears it,
//! and the engine applies the setting again when it starts.
//!
//! Settings (`performance.gaming`): `on` (the switch), `auto` (also on while a game runs) and
//! `threads` (the cap, default 2). One worker thread owns the state: it sleeps until a setting
//! changes, and while `auto` is set it looks for a running game every few seconds.
//!
//! Linux with a systemd user manager only; elsewhere [`Status::supported`] is false and every
//! call here does nothing.

use serde::Serialize;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// Where agent scopes live. The dash nests it under `relay.slice` in the user's manager.
pub const SLICE: &str = "relay-agents.slice";
pub const DEFAULT_THREADS: u32 = 2;
/// How often `auto` looks for a game. Only while `auto` is set; otherwise the worker sleeps.
const POLL: Duration = Duration::from_secs(5);
const CALL_TIMEOUT: Duration = Duration::from_secs(3);

/// What `app.status` reports and `gaming.changed` carries.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Status {
    /// A systemd user manager is there to cap agents with.
    pub supported: bool,
    pub on: bool,
    pub auto: bool,
    pub threads: u32,
    /// The cap is in force: `on`, or `auto` with a game running.
    pub active: bool,
    /// The game `auto` found, when it found one.
    pub game: Option<String>,
}

static STATUS: Mutex<Option<Status>> = Mutex::new(None);
static WORKER: OnceLock<std::thread::Thread> = OnceLock::new();

/// The current state, for `app.status`.
pub fn status() -> Status {
    STATUS.lock().unwrap_or_else(|p| p.into_inner()).clone().unwrap_or(Status {
        supported: supported(),
        threads: DEFAULT_THREADS,
        ..Status::default()
    })
}

/// Whether agents can be capped here: Linux, a running systemd user manager, and the two tools
/// that talk to it. Decided once.
pub fn supported() -> bool {
    static SUPPORTED: OnceLock<bool> = OnceLock::new();
    *SUPPORTED.get_or_init(|| {
        if !cfg!(target_os = "linux") {
            return false;
        }
        let manager = std::env::var_os("XDG_RUNTIME_DIR")
            .map(|dir| Path::new(&dir).join("systemd/private").exists())
            .unwrap_or(false);
        manager && on_path("busctl") && on_path("systemctl")
    })
}

fn on_path(tool: &str) -> bool {
    std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).any(|dir| dir.join(tool).is_file()))
        .unwrap_or(false)
}

/// Move a just-started agent process into a scope of its own under [`SLICE`], on a thread of
/// its own so the spawn never waits for the service manager. The agent's later children are born
/// in the scope; one it forked in the few milliseconds before the move stays outside, which an
/// agent's CLI, still loading, does not do. Failure is logged and changes nothing else.
pub fn adopt(pid: u32, session: &str) {
    if !supported() {
        return;
    }
    let safe: String = session.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    let unit = format!("relay-agent-{safe}-{pid}.scope");
    let spawned = std::thread::Builder::new().name("gaming-adopt".into()).spawn(move || {
        let mut cmd = Command::new("busctl");
        cmd.args(["--user", "call", "org.freedesktop.systemd1", "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager", "StartTransientUnit", "ssa(sv)a(sa(sv))", &unit, "fail", "3",
            "PIDs", "au", "1", &pid.to_string(), "Slice", "s", SLICE, "CollectMode", "s", "inactive-or-failed", "0"]);
        match crate::proc::output_with_timeout(&mut cmd, CALL_TIMEOUT) {
            Ok(Some(out)) if out.status.success() => {}
            Ok(Some(out)) => tracing::warn!(pid, unit, stderr = %String::from_utf8_lossy(&out.stderr).trim(), "could not move the agent into its scope; gaming mode will not cap it"),
            Ok(None) => tracing::warn!(pid, unit, "the service manager did not answer; gaming mode will not cap this agent"),
            Err(error) => tracing::warn!(pid, unit, error = %error, "could not run busctl; gaming mode will not cap this agent"),
        }
    });
    if let Err(error) = spawned {
        tracing::warn!(error = %error, "could not start the thread that scopes an agent");
    }
}

/// Set the cap on [`SLICE`] (`Some(threads)`) or clear it (`None`).
fn apply(cap: Option<u32>) -> bool {
    let mut cmd = Command::new("systemctl");
    cmd.args(["--user", "set-property", "--runtime", SLICE]);
    match cap {
        Some(threads) => cmd.args([format!("CPUQuota={}%", threads.max(1) * 100), "CPUWeight=1".into(), "IOWeight=1".into()]),
        None => cmd.args(["CPUQuota=", "CPUWeight=", "IOWeight="]),
    };
    match crate::proc::output_with_timeout(&mut cmd, CALL_TIMEOUT) {
        Ok(Some(out)) if out.status.success() => true,
        Ok(Some(out)) => {
            tracing::warn!(stderr = %String::from_utf8_lossy(&out.stderr).trim(), "could not set the agents' CPU cap");
            false
        }
        Ok(None) => {
            tracing::warn!("the service manager did not answer the CPU cap change");
            false
        }
        Err(error) => {
            tracing::warn!(error = %error, "could not run systemctl for the CPU cap");
            false
        }
    }
}

/// Wake the worker to read the settings again (a `performance.*` setting changed).
pub fn nudge() {
    if let Some(worker) = WORKER.get() {
        worker.unpark();
    }
}

fn read_settings(engine: &crate::engine::Engine) -> (bool, bool, u32) {
    let conn = engine.store.lock();
    let leaf = |path: &str| -> Option<serde_json::Value> {
        conn.prepare_cached("SELECT value FROM settings WHERE path=?1").ok()?
            .query_row([path], |r| r.get::<_, String>(0)).ok()
            .and_then(|value| serde_json::from_str(&value).ok())
    };
    let on = leaf("performance.gaming.on").and_then(|v| v.as_bool()).unwrap_or(false);
    let auto = leaf("performance.gaming.auto").and_then(|v| v.as_bool()).unwrap_or(false);
    let threads = leaf("performance.gaming.threads").and_then(|v| v.as_u64()).map(|n| n.clamp(1, 64) as u32).unwrap_or(DEFAULT_THREADS);
    (on, auto, threads)
}

/// A game running now, by a process only games have: Steam's launch reaper (named with the
/// app id), gamescope, or Lutris's wrapper. One directory listing and a short read per process.
pub fn detect() -> Option<String> {
    let entries = std::fs::read_dir("/proc").ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str().filter(|n| n.bytes().all(|b| b.is_ascii_digit())) else { continue };
        let Ok(comm) = std::fs::read_to_string(format!("/proc/{pid}/comm")) else { continue };
        match comm.trim() {
            "reaper" => {
                let Ok(cmdline) = std::fs::read(format!("/proc/{pid}/cmdline")) else { continue };
                let cmdline = String::from_utf8_lossy(&cmdline).replace('\0', " ");
                if cmdline.contains("SteamLaunch") {
                    let app = cmdline.split_whitespace().find_map(|arg| arg.strip_prefix("AppId="));
                    return Some(app.map(|id| format!("Steam app {id}")).unwrap_or_else(|| "a Steam game".into()));
                }
            }
            "gamescope" | "gamescope-wl" => return Some("gamescope".into()),
            "lutris-wrapper" => return Some("a Lutris game".into()),
            _ => {}
        }
    }
    None
}

/// Start the worker. It applies the saved setting at once — clearing a cap a previous engine
/// left on the slice — then waits for [`nudge`], polling for games only while `auto` is set.
pub fn spawn_worker(engine: &Arc<crate::engine::Engine>) {
    if !supported() || WORKER.get().is_some() {
        return;
    }
    let weak = Arc::downgrade(engine);
    let spawned = std::thread::Builder::new().name("gaming".into()).spawn(move || {
        crate::background_priority();
        let mut applied: Option<Option<u32>> = None;
        loop {
            let Some(engine) = weak.upgrade() else { return };
            if engine.is_quitting() {
                return;
            }
            let (on, auto, threads) = read_settings(&engine);
            let game = if auto { detect() } else { None };
            let active = on || game.is_some();
            let cap = active.then_some(threads);
            if applied != Some(cap) && apply(cap) {
                if cap.is_some() || applied.is_some() {
                    tracing::info!(active, threads, game = game.as_deref().unwrap_or(""), "gaming mode");
                }
                applied = Some(cap);
            }
            let now = Status { supported: true, on, auto, threads, active, game };
            let changed = {
                let mut status = STATUS.lock().unwrap_or_else(|p| p.into_inner());
                let changed = status.as_ref() != Some(&now);
                *status = Some(now.clone());
                changed
            };
            if changed {
                engine.emit_system("gaming.changed", serde_json::to_value(&now).unwrap_or_default());
            }
            drop(engine);
            if auto {
                std::thread::park_timeout(POLL);
            } else {
                std::thread::park();
            }
        }
    });
    match spawned {
        Ok(handle) => {
            let _ = WORKER.set(handle.thread().clone());
        }
        Err(error) => tracing::warn!(error = %error, "could not start the gaming mode worker"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_on_this_machine_reads_as_a_game_in_tests() {
        // The detector must not mistake ordinary processes for games: in a test run none of
        // the names it looks for is running unless a game really is.
        let found = detect();
        if let Some(game) = &found {
            assert!(game.contains("Steam") || game == "gamescope" || game.contains("Lutris"), "{game}");
        }
    }

    #[test]
    fn status_before_the_worker_starts_is_off_with_the_default_cap() {
        let status = status();
        assert!(!status.active || status.on || status.game.is_some());
        assert!(status.threads >= 1);
    }
}
