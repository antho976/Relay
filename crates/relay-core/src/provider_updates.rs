//! Opt-in self-updates for known user-owned installations, outside the store lock.
use crate::engine::{Ctx, Engine, IntoBus};
use relay_bus::ops::provider::{Update, UpdateOut};
use relay_bus::types::Provider;
use relay_bus::BusError;
use rusqlite::params;
use serde_json::json;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

fn installation(provider: Provider, path: &Path, home: &Path) -> Option<&'static str> {
    match provider {
        Provider::Claude if path.starts_with(home.join(".local/share/claude/versions")) => {
            Some("native")
        }
        Provider::Codex if path.starts_with(home.join(".codex/packages/standalone/releases")) => {
            Some("standalone")
        }
        _ => None,
    }
}

/// The provider as its notice and answer name it in a sentence, as the apps do.
fn title(provider: Provider) -> &'static str {
    match provider {
        Provider::Claude => "Claude Code",
        Provider::Codex => "Codex",
        Provider::Shell => "Shell",
    }
}

pub(crate) fn register(engine: &mut Engine) {
    engine.register::<Update>(|ctx: &mut Ctx, payload| {
        let provider = payload.provider;
        let name = crate::sessions::provider_str(provider);
        let title = title(provider);
        if payload.automatic == Some(true)
            && crate::handlers::settings::get(ctx.tx(), Some(&format!("providers.{name}.auto_update")))? != true
        {
            return Ok(UpdateOut { started: false, method: "disabled".into(), message: "Startup updates are disabled for this provider.".into() });
        }
        let path = crate::providers::executable(ctx.tx(), provider)?;
        let path = std::fs::canonicalize(&path).map_err(|e| BusError::unavailable("provider.update_path", e.to_string()))?;
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
        let method = installation(provider, &path, &home);
        if home.as_os_str().is_empty() || method.is_none() || path.metadata().map(|m| m.uid()).ok() != Some(unsafe { libc::geteuid() }) {
            return Ok(UpdateOut { started: false, method: "managed_or_unknown".into(), message: "Update this installation with its package manager or installer. Relay only self-updates known user-owned native installations.".into() });
        }
        // A row with a pid has a process on the binary. Nothing marks a launch that is still
        // being prepared (no row is ever `spawning`, RA-622), so that short window is not seen.
        let active: bool = ctx.tx().query_row(
            "SELECT EXISTS(SELECT 1 FROM sessions WHERE provider=?1 AND pid IS NOT NULL AND state NOT IN ('closed','exited'))",
            [name], |row| row.get(0),
        ).bus()?;
        if active {
            return Ok(UpdateOut { started: false, method: method.unwrap().into(), message: "Update skipped while this provider has a live session. Existing sessions are unchanged.".into() });
        }
        if !ctx.engine().provider_updates.lock().unwrap().insert(name.into()) {
            return Ok(UpdateOut { started: false, method: method.unwrap().into(), message: "An update is already running for this provider.".into() });
        }
        ctx.emit("provider.update.changed", json!({"provider":name,"state":"updating","message":"Checking for an update…"}));
        ctx.after_commit(move |engine| {
            std::thread::spawn(move || {
                let result = run(&path, Duration::from_secs(120));
                let (state, message) = match &result {
                    Ok(message) => ("complete", message.clone()),
                    Err(message) => ("failed", message.clone()),
                };
                // Probe the updated binaries before taking the store: `--version` and the auth
                // check are subprocesses, and only their answers belong inside the write (D149).
                let probes = result.is_ok().then(|| {
                    let paths = crate::providers::paths(&engine.store.lock());
                    crate::providers::probe(paths)
                });
                let _ = engine.system_write("provider.update.finished", None, None, None, json!({"provider":name,"state":state}), |tx, now| {
                    if let Some(probes) = probes { let _ = crate::providers::record(tx, now, probes); }
                    tx.execute("INSERT INTO notifications(project_id,category,title,body,link,read,created_at) VALUES (NULL,'provider',?1,?2,NULL,0,?3)",
                        params![format!("{title} update {state}"),message,now]).bus()?;
                    Ok(((), vec![
                        ("provider.update.changed".into(), json!({"provider":name,"state":state,"message":message})),
                        ("notify.new".into(), json!({"category":"provider"})),
                    ]))
                });
                engine.provider_updates.lock().unwrap().remove(name);
            });
        });
        Ok(UpdateOut { started: true, method: method.unwrap().into(), message: format!("Updating {title} in the background. New sessions use the updated version.") })
    });
}

/// Run `<path> update` through the shared bounded runner (SIGTERM then SIGKILL of the whole
/// group at the deadline, drains that never outwait it) and keep only the message shaping here.
/// The message is the outcome line: stdout's last on success, stderr's on failure, since an
/// updater's progress and its errors go to stderr while its result goes to stdout (RA-327).
fn run(path: &Path, timeout: Duration) -> Result<String, String> {
    let output = crate::proc::output_with_timeout(Command::new(path).arg("update"), timeout)
        .map_err(|e| format!("Could not start update: {e}"))?
        .ok_or_else(|| "Update timed out. Check the provider version before retrying.".to_string())?;
    let last = |bytes: &[u8]| {
        String::from_utf8_lossy(bytes).lines().rev().map(str::trim).find(|line| !line.is_empty())
            .map(|line| line.chars().take(1000).collect::<String>())
    };
    if output.status.success() {
        Ok(last(&output.stdout).or_else(|| last(&output.stderr)).unwrap_or_else(|| "Update command completed.".into()))
    } else {
        let message = last(&output.stderr).or_else(|| last(&output.stdout)).unwrap_or_else(|| output.status.to_string());
        Err(format!("Update failed: {message}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::time::Instant;
    fn script(root: &Path, body: &str) -> PathBuf {
        let path = root.join("provider");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }
    #[test]
    fn installation_detection_does_not_treat_npm_or_system_paths_as_native() {
        let home = Path::new("/home/fixture");
        assert_eq!(
            installation(
                Provider::Claude,
                &home.join(".local/share/claude/versions/1.2.3"),
                home
            ),
            Some("native")
        );
        assert_eq!(
            installation(
                Provider::Codex,
                &home.join(".codex/packages/standalone/releases/v1/bin/codex"),
                home
            ),
            Some("standalone")
        );
        for path in [
            "/usr/bin/codex",
            "/home/fixture/.npm/bin/codex",
            "/home/fixture/.local/share/claude/versions-other/custom",
        ] {
            assert!(installation(Provider::Codex, Path::new(path), home).is_none());
            assert!(installation(Provider::Claude, Path::new(path), home).is_none());
        }
    }
    #[test]
    fn updater_executes_only_update_and_captures_failures() {
        let root = tempfile::tempdir().unwrap();
        let path = script(
            root.path(),
            "[ \"$1\" = update ] || exit 4\n[ \"$#\" = 1 ] || exit 5\necho updated",
        );
        assert_eq!(run(&path, Duration::from_secs(2)).unwrap(), "updated");
        script(root.path(), "echo download-failed >&2\nexit 9");
        assert!(run(&path, Duration::from_secs(2))
            .unwrap_err()
            .contains("download-failed"));
        // Progress on stderr does not stand in for the outcome on stdout, nor the reverse.
        script(root.path(), "echo 'Updated to 2.0'\necho 'downloading 100%' >&2");
        assert_eq!(run(&path, Duration::from_secs(2)).unwrap(), "Updated to 2.0");
        script(root.path(), "echo 'checking' \necho 'no space left' >&2\necho 'cleaning up'\nexit 1");
        assert_eq!(run(&path, Duration::from_secs(2)).unwrap_err(), "Update failed: no space left");
    }
    #[test]
    fn updater_kills_a_hung_process_group_and_bounds_output() {
        // Both bounds below are deliberately far looser than the work they wrap. What this
        // test proves is that a hung group is killed rather than waited on, and that output is
        // truncated — neither claim is about how fast the machine is. Tight wall-clock budgets
        // made this the one test that failed at random when the suite runs in parallel, and a
        // test that fails at random teaches people to ignore the suite.
        let root = tempfile::tempdir().unwrap();
        let path = script(root.path(), "sleep 60 &\nwait");
        let started = Instant::now();
        assert!(run(&path, Duration::from_millis(100))
            .unwrap_err()
            .contains("timed out"));
        // The child sleeps for 60s; returning in under 10 proves it was not waited on.
        assert!(started.elapsed() < Duration::from_secs(10));
        script(root.path(), "head -c 200000 /dev/zero | tr '\\000' x");
        // Milliseconds of work. The timeout only has to be long enough not to fire.
        assert!(run(&path, Duration::from_secs(30)).unwrap().len() <= 1000);
    }

    #[test]
    fn registered_update_defaults_to_disabled_and_never_runs_an_unknown_install() {
        let root = tempfile::tempdir().unwrap();
        let store = crate::Store::open(&root.path().join("store.db"), false).unwrap();
        let engine = Engine::new(crate::Instance::Test, store);
        let call = |op, payload| {
            engine
                .dispatch(
                    relay_bus::Request::new(relay_bus::Actor::User, op, payload),
                    crate::Door::InProcess,
                )
                .into_result()
                .unwrap()
        };
        assert_eq!(
            call(
                "provider.update",
                json!({"provider":"codex","automatic":true})
            )["method"],
            "disabled"
        );
        let marker = root.path().join("was-run");
        let path = script(root.path(), &format!("touch '{}'", marker.display()));
        call(
            "settings.set",
            json!({"path":"providers.codex.path","value":path}),
        );
        let result = call("provider.update", json!({"provider":"codex"}));
        assert_eq!(result["started"], false);
        assert_eq!(result["method"], "managed_or_unknown");
        assert!(
            !marker.exists(),
            "Unsupported installations must never execute an updater"
        );
    }
}
