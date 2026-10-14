//! The Blender plugin's children under bubblewrap (RA-077).
//!
//! `blender_python` runs whatever Python the agent sends, as the user. The guardrail gate
//! decides whether it runs at all (mcp::plugin_gate); this decides what it can touch once it
//! does. Inside a Relay session on Linux with a working `bwrap`, every Blender child sees the
//! whole file system read-only and may write only to the session's write roots (the guardrail's
//! own list, `bus.whoami`), the job's folder and Blender's sandbox config/cache. The network is
//! off: no tool, add-ons included, needs it. GPU devices stay reachable for EEVEE and Cycles.
//! The user's runtime folder and the X socket folder are hidden (their sockets would reach the
//! session bus and the service manager, which run commands outside the sandbox), the child gets
//! a session of its own, and only an allowlist of the environment (`passed_env`).
//!
//! Outside a session (a standalone plugin) or without bwrap (macOS, Windows, a kernel without
//! user namespaces) Blender runs as before, and every result says so in `sandbox`: the sandbox
//! is never silently off.
//!
//! What it does not stop: reading anything the user can read, and anything written inside the
//! write roots, which is the worktree the agent may edit anyway.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

pub(crate) enum Sandbox {
    /// Children run under bwrap and may write to these folders, besides their own job's.
    On { bwrap: PathBuf, writable: Vec<PathBuf>, from: String },
    Off(String),
}

impl Sandbox {
    /// The sandbox for a Blender child of this server. `checkout` is the fallback write root
    /// when the session's own list cannot be read.
    pub(crate) fn current(checkout: &Path) -> Sandbox {
        #[cfg(test)]
        if let Some(writable) = TEST_ROOTS.with(|roots| roots.borrow().clone()) {
            return match bwrap() {
                Ok(bwrap) => Sandbox::On { bwrap, writable, from: "test roots".into() },
                Err(why) => Sandbox::Off(why),
            };
        }
        if std::env::var("RELAY_SESSION").map_or(true, |s| s.is_empty()) {
            return Sandbox::Off("not a Relay session; Blender runs unconfined".into());
        }
        if cfg!(test) {
            // Unit tests run inside agent sessions; they must not reach the live engine.
            return Sandbox::Off("unit test".into());
        }
        let bwrap = match bwrap() {
            Ok(bwrap) => bwrap,
            Err(why) => {
                eprintln!("relay blender-mcp: {why}");
                return Sandbox::Off(why);
            }
        };
        match crate::mcp::session_write_roots() {
            Ok(roots) if !roots.is_empty() => Sandbox::On { bwrap, writable: roots, from: "the session's guardrail write roots".into() },
            Ok(_) => Sandbox::On { bwrap, writable: vec![checkout.to_path_buf()], from: "the worktree (the session reported no write roots)".into() },
            Err(error) => Sandbox::On {
                bwrap,
                writable: vec![checkout.to_path_buf()],
                from: format!("the worktree only (write roots unavailable: {error:#})"),
            },
        }
    }

    /// `command` (a Blender invocation) as this sandbox runs it. `job` holds the script and is
    /// the child's TMPDIR; `extra` are further folders this one run writes (a render's output).
    pub(crate) fn wrap(&self, command: Command, job: &Path, extra: &[PathBuf]) -> Result<Command> {
        let Sandbox::On { bwrap, writable, .. } = self else { return Ok(command) };
        let state = state_dir()?;
        let (config, cache) = (state.join("config"), state.join("cache"));
        for dir in [&config, &cache] {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let mut sandboxed = Command::new(bwrap);
        // --new-session: no TIOCSTI into a terminal the server was started from.
        sandboxed.args(["--ro-bind", "/", "/", "--dev", "/dev", "--proc", "/proc", "--unshare-net", "--unshare-pid", "--die-with-parent", "--new-session"]);
        for device in gpu_devices() {
            sandboxed.arg("--dev-bind-try").arg(&device).arg(&device);
        }
        // A read-only file system still lets a child connect() to the sockets on it: the session
        // bus and the systemd user manager (which starts any command, unconfined, on request), the
        // Wayland and X servers, ssh and gpg agents. Empty folders go over those first.
        for dir in hidden_socket_dirs() {
            sandboxed.arg("--tmpfs").arg(&dir);
        }
        let mut bound = Vec::new();
        for dir in writable.iter().chain(extra).chain([&job.to_path_buf(), &state]) {
            // A root that does not exist yet has nothing to write into; binding it would fail.
            let Ok(real) = std::fs::canonicalize(dir) else { continue };
            if !bound.contains(&real) {
                sandboxed.arg("--bind").arg(&real).arg(&real);
                bound.push(real);
            }
        }
        if let Some(cwd) = command.get_current_dir() {
            sandboxed.arg("--chdir").arg(cwd);
        }
        // Not the server's environment: it names the session bus, agent sockets and Relay's own
        // tokens. Only what Blender, its GPU drivers and the locale read is passed on.
        sandboxed.env_clear();
        for (key, value) in std::env::vars_os() {
            if key.to_str().is_some_and(passed_env) {
                sandboxed.env(key, value);
            }
        }
        for (key, value) in command.get_envs() {
            match value {
                Some(value) => sandboxed.env(key, value),
                None => sandboxed.env_remove(key),
            };
        }
        // Blender's config and caches (its own, Mesa's, NVIDIA's) go to the sandbox's folder, and
        // its temp files to the job's.
        sandboxed.env("XDG_CONFIG_HOME", &config).env("XDG_CACHE_HOME", &cache).env("TMPDIR", job);
        sandboxed.arg("--").arg(command.get_program()).args(command.get_args());
        Ok(sandboxed)
    }

    /// One line for the tool result: whether the child was confined, and to what.
    pub(crate) fn describe(&self) -> String {
        match self {
            Sandbox::On { writable, from, .. } => format!(
                "bwrap: read-only file system, no network; writes allowed only in {} ({from}) and the job's own folders",
                writable.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
            ),
            Sandbox::Off(why) => format!("off: {why}"),
        }
    }
}

/// A working bwrap, found and tried once per server. Installed is not enough: a kernel or
/// distribution that forbids unprivileged user namespaces makes every run fail.
fn bwrap() -> Result<PathBuf, String> {
    static FOUND: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    FOUND.get_or_init(|| {
        if !cfg!(target_os = "linux") {
            return Err("the Blender sandbox needs bubblewrap, which is Linux-only; Blender runs unconfined".into());
        }
        let bwrap = std::env::var_os("PATH")
            .and_then(|paths| std::env::split_paths(&paths).map(|dir| dir.join("bwrap")).find(|p| p.is_file()))
            .or_else(|| Some(PathBuf::from("/usr/bin/bwrap")).filter(|p| p.is_file()))
            .ok_or("bubblewrap (bwrap) is not installed; Blender runs unconfined")?;
        let tried = relay_core::proc::output_with_timeout(
            Command::new(&bwrap).args(["--ro-bind", "/", "/", "--dev", "/dev", "--proc", "/proc", "--unshare-net", "--unshare-pid", "--die-with-parent", "true"]),
            Duration::from_secs(10),
        );
        match tried {
            Ok(Some(output)) if output.status.success() => Ok(bwrap),
            Ok(Some(output)) => Err(format!("bwrap does not work here ({}); Blender runs unconfined", String::from_utf8_lossy(&output.stderr).trim())),
            Ok(None) => Err("bwrap did not start within 10 s; Blender runs unconfined".into()),
            Err(error) => Err(format!("bwrap could not run ({error}); Blender runs unconfined")),
        }
    }).clone()
}

/// Environment variables a sandboxed Blender keeps: its executable search and libraries, the
/// locale, Blender's own settings and the GPU drivers'. Nothing that names a socket or a secret.
fn passed_env(key: &str) -> bool {
    const EXACT: [&str; 11] = ["PATH", "HOME", "USER", "LOGNAME", "LANG", "LANGUAGE", "TZ", "TERM", "LD_LIBRARY_PATH", "OCIO", "DRI_PRIME"];
    const PREFIXES: [&str; 14] = ["LC_", "BLENDER_", "__GL", "__EGL", "__NV", "__VK", "MESA_", "LIBGL_", "VK_", "EGL_", "GBM_", "CUDA_", "OPTIX_", "NVIDIA_"];
    const NEVER: [&str; 6] = ["SOCK", "TOKEN", "SECRET", "PASSWORD", "AUTH", "KEY"];
    let upper = key.to_ascii_uppercase();
    (EXACT.contains(&key) || PREFIXES.iter().any(|p| key.starts_with(p))) && !NEVER.iter().any(|n| upper.contains(n))
}

/// The user's runtime folder (`/run/user/<uid>`, and `XDG_RUNTIME_DIR` where that differs) and
/// the X server's socket folder: each one that exists, since bwrap cannot make a mount point on
/// the read-only root.
fn hidden_socket_dirs() -> Vec<PathBuf> {
    use std::os::unix::fs::MetadataExt;
    let mut dirs: Vec<PathBuf> = std::fs::metadata("/proc/self").map(|m| PathBuf::from(format!("/run/user/{}", m.uid()))).into_iter().collect();
    dirs.extend(std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).filter(|p| p.is_absolute()));
    dirs.push(PathBuf::from("/tmp/.X11-unix"));
    let mut real: Vec<PathBuf> = Vec::new();
    for dir in dirs.iter().filter_map(|dir| std::fs::canonicalize(dir).ok()) {
        if dir.is_dir() && dir != Path::new("/") && !real.contains(&dir) {
            real.push(dir);
        }
    }
    real
}

/// Kept across runs, so shader caches survive; never the user's own Blender config.
fn state_dir() -> Result<PathBuf> {
    let cache = std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .context("no cache directory for the Blender sandbox (HOME is unset)")?;
    Ok(cache.join("relay").join("blender-sandbox"))
}

/// Render devices: DRM nodes for EEVEE and Mesa, the NVIDIA nodes for CUDA/OptiX and its EGL.
fn gpu_devices() -> Vec<PathBuf> {
    let mut devices = vec![PathBuf::from("/dev/dri")];
    if let Ok(entries) = std::fs::read_dir("/dev") {
        devices.extend(entries.flatten().map(|e| e.path()).filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("nvidia"))));
    }
    devices.sort();
    devices
}

#[cfg(test)]
thread_local! {
    static TEST_ROOTS: std::cell::RefCell<Option<Vec<PathBuf>>> = const { std::cell::RefCell::new(None) };
}

/// Sandbox the Blender runs this test thread makes, with `roots` writable, until dropped.
#[cfg(test)]
pub(crate) fn test_roots(roots: &[&Path]) -> impl Drop {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            TEST_ROOTS.with(|r| *r.borrow_mut() = None);
        }
    }
    TEST_ROOTS.with(|r| *r.borrow_mut() = Some(roots.iter().map(|p| p.to_path_buf()).collect()));
    Reset
}

#[cfg(test)]
pub(crate) fn available() -> bool {
    bwrap().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sandbox_passes_no_sockets_or_secrets_in_the_environment() {
        for kept in ["PATH", "HOME", "LANG", "LC_ALL", "BLENDER_USER_SCRIPTS", "__GLX_VENDOR_LIBRARY_NAME", "MESA_LOADER_DRIVER_OVERRIDE", "CUDA_VISIBLE_DEVICES"] {
            assert!(passed_env(kept), "{kept}");
        }
        for dropped in ["DBUS_SESSION_BUS_ADDRESS", "SSH_AUTH_SOCK", "GPG_AGENT_INFO", "XDG_RUNTIME_DIR", "WAYLAND_DISPLAY", "DISPLAY",
            "RELAY_SOCKET", "RELAY_TOKEN", "RELAY_SESSION", "ANTHROPIC_API_KEY", "BLENDER_API_KEY", "LC_AUTH_TOKEN", "GITHUB_TOKEN"] {
            assert!(!passed_env(dropped), "{dropped}");
        }
    }
}
