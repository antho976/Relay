//! The Unreal editor as a process: which engine a project builds with, whether an editor (or a
//! leftover crash reporter) is running, whether the Remote Control port is free, launching and
//! reading the editor log for bind failures, and the hot-reload module check after a build.
//!
//! Every lesson here came from a real session: a missing `UE_ROOT` stopped every build; a
//! crash reporter left behind made UnrealBuildTool build a numbered hot-reload module the
//! editor never loaded; and a quick relaunch lost the Remote Control port for a while.

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// A running process, as `/proc` shows it (Linux only; empty elsewhere).
#[derive(Debug, Clone)]
pub struct Proc {
    pub pid: u32,
    pub exe: PathBuf,
    pub cmdline: Vec<String>,
}

impl Proc {
    pub fn name(&self) -> String {
        self.exe.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    }
}

pub fn processes() -> Vec<Proc> {
    let Ok(entries) = std::fs::read_dir("/proc") else { return Vec::new() };
    entries
        .flatten()
        .filter_map(|e| e.file_name().to_string_lossy().parse::<u32>().ok())
        .filter_map(|pid| {
            let exe = std::fs::read_link(format!("/proc/{pid}/exe")).ok()?;
            let raw = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
            let cmdline = raw.split(|b| *b == 0).filter(|a| !a.is_empty()).map(|a| String::from_utf8_lossy(a).into_owned()).collect();
            Some(Proc { pid, exe, cmdline })
        })
        .collect()
}

fn is_editor(p: &Proc) -> bool {
    let name = p.name();
    (name == "UnrealEditor" || name == "UnrealEditor.exe" || name == "UE4Editor")
        && !p.cmdline.iter().any(|a| a.to_ascii_lowercase().contains("-run=") || a.eq_ignore_ascii_case("-nullrhi"))
}

fn is_crash_reporter(p: &Proc) -> bool {
    p.name().starts_with("CrashReportClient")
}

/// Editors that have this project open: this exact file, not another checkout's copy of it or
/// a project whose name merely ends the same way. Signals go to these PIDs.
pub fn editors_for(uproject: &Path) -> Vec<Proc> {
    let want = uproject.canonicalize().unwrap_or_else(|_| uproject.to_path_buf());
    processes()
        .into_iter()
        .filter(is_editor)
        .filter(|p| opens(p, &want))
        .collect()
}

/// Whether an editor process opened the canonical `want`. A relative argument is resolved
/// against the PWD the editor was started with, not its cwd: UnrealEditor changes into
/// Engine/Binaries at start-up. An argument that cannot be resolved does not match.
fn opens(p: &Proc, want: &Path) -> bool {
    let pwd = std::fs::read(format!("/proc/{}/environ", p.pid)).ok().and_then(|raw| {
        raw.split(|b| *b == 0).find_map(|var| var.strip_prefix(b"PWD=")).map(|v| PathBuf::from(String::from_utf8_lossy(v).into_owned()))
    });
    project_args(&p.cmdline, pwd.as_deref()).iter().any(|path| path == want)
}

/// The canonical .uproject paths named on a command line.
fn project_args(cmdline: &[String], pwd: Option<&Path>) -> Vec<PathBuf> {
    cmdline
        .iter()
        .skip(1)
        .filter(|a| a.to_ascii_lowercase().ends_with(".uproject"))
        .filter_map(|a| {
            let path = Path::new(a);
            let path = if path.is_absolute() { path.to_path_buf() } else { pwd?.join(path) };
            path.canonicalize().ok()
        })
        .collect()
}

/// Whether `pid` is still an editor with this project open, checked right before a signal so a
/// PID reused since the list was taken is never hit.
pub fn still_editor_for(pid: u32, uproject: &Path) -> bool {
    editors_for(uproject).iter().any(|p| p.pid == pid)
}

pub fn crash_reporters() -> Vec<Proc> {
    processes().into_iter().filter(is_crash_reporter).collect()
}

pub fn kill(pid: u32) -> bool {
    std::process::Command::new("kill").arg(pid.to_string()).status().is_ok_and(|s| s.success())
}

// ---------------------------------------------------------------- which engine

/// `(major, minor)` from an engine's `Engine/Build/Build.version`.
fn engine_version(root: &Path) -> Option<(u64, u64)> {
    let raw = std::fs::read_to_string(root.join("Engine/Build/Build.version")).ok()?;
    let v: Value = serde_json::from_str(&raw).ok()?;
    Some((v["MajorVersion"].as_u64()?, v["MinorVersion"].as_u64()?))
}

fn strip_braces(s: &str) -> String {
    s.trim().trim_start_matches('{').trim_end_matches('}').to_ascii_lowercase()
}

/// Engine folders registered in Epic's `Install.ini` (launcher and source builds on Linux and
/// macOS), as `(key, path)`.
fn registered_engines() -> Vec<(String, PathBuf)> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else { return Vec::new() };
    let mut out = Vec::new();
    for ini in [home.join(".config/Epic/UnrealEngine/Install.ini"), home.join("Library/Application Support/Epic/UnrealEngine/Install.ini")] {
        let Ok(text) = std::fs::read_to_string(&ini) else { continue };
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('[') || line.starts_with(';') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let path = PathBuf::from(value.trim().trim_matches('"'));
                if !key.trim().is_empty() {
                    out.push((key.trim().to_string(), path));
                }
            }
        }
    }
    out
}

/// The engine directory (the one holding `Engine/`) for a project, trying in order: `UE_ROOT`;
/// an editor already running this project; the project's own log ("Base Directory"); a source
/// tree around the project; `Install.ini` by association key (braces optional) and then by
/// version; common install folders. The error lists everything tried.
pub fn engine_root(uproject: &Path, project_root: &Path, association: &str, log: &Path) -> Result<PathBuf> {
    let valid = |p: &Path| p.join("Engine/Build/BatchFiles").is_dir();
    let mut tried: Vec<String> = Vec::new();
    if let Some(root) = std::env::var_os("UE_ROOT").map(PathBuf::from) {
        if valid(&root) {
            return Ok(root);
        }
        bail!("UE_ROOT={} has no Engine/Build/BatchFiles", root.display());
    }
    // An editor that is running this project knows exactly which engine it is.
    for editor in editors_for(uproject) {
        if let Some(root) = editor.exe.ancestors().find(|a| valid(a)) {
            return Ok(root.to_path_buf());
        }
    }
    tried.push("a running editor for this project".into());
    // The last editor session logged its binary directory, near the top: only the head is read,
    // however long the log has grown (RA-287).
    if let Ok(file) = std::fs::File::open(log) {
        use std::io::BufRead;
        let mut head = std::io::BufReader::new(file);
        let mut raw = Vec::new();
        for _ in 0..400 {
            raw.clear();
            if !matches!(head.read_until(b'\n', &mut raw), Ok(n) if n > 0) {
                break;
            }
            let line = String::from_utf8_lossy(&raw);
            if let Some(rest) = line.split("Base Directory:").nth(1) {
                let dir = PathBuf::from(rest.trim());
                if let Some(root) = dir.ancestors().find(|a| valid(a)) {
                    return Ok(root.to_path_buf());
                }
            }
        }
    }
    tried.push(format!("\"Base Directory\" in {}", log.display()));
    for dir in project_root.ancestors().skip(1) {
        if valid(dir) {
            return Ok(dir.to_path_buf());
        }
    }
    tried.push("a source engine tree around the project".into());
    let registered = registered_engines();
    let want = strip_braces(association);
    if let Some((_, path)) = registered.iter().find(|(key, path)| strip_braces(key) == want && valid(path)) {
        return Ok(path.clone());
    }
    // "5.8" in the .uproject but the engine registered under a GUID: match by version.
    let wanted_version = association.split('.').map(|p| p.parse::<u64>().ok()).collect::<Option<Vec<_>>>()
        .filter(|v| v.len() >= 2).map(|v| (v[0], v[1]));
    if let Some(version) = wanted_version {
        if let Some((_, path)) = registered.iter().find(|(_, path)| valid(path) && engine_version(path) == Some(version)) {
            return Ok(path.clone());
        }
    }
    tried.push(format!("Install.ini entries: {}", if registered.is_empty() { "none".to_string() } else { registered.iter().map(|(k, p)| format!("{k}={}", p.display())).collect::<Vec<_>>().join(", ") }));
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        for base in [home.clone(), home.join("dev"), home.join("Epic"), home.join("UnrealEngine")] {
            if let Ok(entries) = std::fs::read_dir(&base) {
                for e in entries.flatten() {
                    let n = e.file_name().to_string_lossy().to_lowercase();
                    if n.contains("unreal") || n.starts_with("ue_") || n.starts_with("ue5") || n.starts_with("ue-") {
                        candidates.push(e.path());
                    }
                }
            }
        }
        candidates.push(home.join("UnrealEngine"));
    }
    for base in ["/opt", "/Users/Shared/Epic Games", "C:/Program Files/Epic Games"] {
        if let Ok(entries) = std::fs::read_dir(base) {
            candidates.extend(entries.flatten().map(|e| e.path()));
        }
    }
    let mut found: Vec<PathBuf> = candidates.into_iter().filter(|c| valid(c)).collect();
    found.sort();
    found.dedup();
    if let Some(version) = wanted_version {
        if let Some(root) = found.iter().find(|p| engine_version(p) == Some(version)) {
            return Ok(root.clone());
        }
    }
    // A single installed engine is only a safe guess when the project names no version.
    if found.len() == 1 && wanted_version.is_none() {
        return Ok(found.remove(0));
    }
    tried.push(format!("common folders under ~, ~/dev, /opt ({} engines found{})", found.len(),
        if found.is_empty() { String::new() } else { format!(": {}", found.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")) }));
    Err(anyhow!(
        "cannot find the engine for EngineAssociation {association:?}. Tried: {}. Set UE_ROOT to the folder that contains Engine/ (in the environment Relay's engine starts from), or open the project in the editor once so its log records the engine path.",
        tried.join("; ")
    ))
}

pub fn editor_binary(engine: &Path) -> PathBuf {
    let bin = engine.join("Engine/Binaries");
    if cfg!(target_os = "windows") {
        bin.join("Win64/UnrealEditor.exe")
    } else if cfg!(target_os = "macos") {
        bin.join("Mac/UnrealEditor.app/Contents/MacOS/UnrealEditor")
    } else {
        bin.join("Linux/UnrealEditor")
    }
}

// ---------------------------------------------------------------- before and after a build

/// Whether a crash reporter is this project's: its command line names a path inside the
/// project's folder (the crash folder under Saved/Crashes, or the .uproject).
fn reports_for(p: &Proc, uproject: &Path) -> bool {
    let Some(root) = uproject.parent().filter(|r| !r.as_os_str().is_empty()) else { return false };
    let roots: Vec<String> = [root.to_path_buf(), root.canonicalize().unwrap_or_else(|_| root.to_path_buf())]
        .iter().map(|r| r.to_string_lossy().trim_end_matches('/').to_string()).filter(|r| !r.is_empty()).collect();
    p.cmdline.iter().skip(1).any(|a| roots.iter().any(|r| a.contains(&format!("{r}/")) || a.trim_matches('"') == r))
}

/// What stands between a build and an editor that runs the new code. A leftover crash reporter
/// makes UnrealBuildTool believe an editor is running and build a hot-reload module instead.
/// Only this project's reporters are stopped: another project's crash dialog may still be
/// waiting on its human (RA-293), so those are reported and left alone.
pub fn prebuild(uproject: &Path, kill_crash_reporters: bool) -> Value {
    let editors = editors_for(uproject);
    let reporters = crash_reporters();
    let (ours, others): (Vec<&Proc>, Vec<&Proc>) = reporters.iter().partition(|r| reports_for(r, uproject));
    let mut killed = Vec::new();
    if kill_crash_reporters {
        for r in &ours {
            if kill(r.pid) {
                killed.push(r.pid);
            }
        }
        if !killed.is_empty() {
            std::thread::sleep(Duration::from_millis(500));
        }
    }
    json!({
        "editor_running": editors.iter().map(|p| p.pid).collect::<Vec<_>>(),
        "crash_reporters": reporters.iter().map(|p| p.pid).collect::<Vec<_>>(),
        "killed_crash_reporters": killed,
        "other_crash_reporters": others.iter().map(|p| p.pid).collect::<Vec<_>>(),
    })
}

/// Modules the editor loads, from `Binaries/<Platform>/UnrealEditor.modules`, and whether any is
/// a numbered hot-reload build (`...-MyGame-0001.so`) the editor may not pick up on restart.
pub fn module_manifest(project_root: &Path, platform: &str) -> Value {
    let path = project_root.join("Binaries").join(platform).join("UnrealEditor.modules");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return json!({"manifest": path, "found": false});
    };
    let manifest: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
    let re = regex::Regex::new(r"-\d{4}(\.[A-Za-z]+)?$").unwrap();
    let modules = manifest["Modules"].as_object().cloned().unwrap_or_default();
    let numbered: Vec<String> = modules
        .iter()
        .filter_map(|(name, file)| {
            let f = file.as_str()?;
            let stem = Path::new(f).file_stem()?.to_string_lossy().into_owned();
            re.is_match(&stem).then(|| format!("{name} -> {f}"))
        })
        .collect();
    json!({"manifest": path, "found": true, "build_id": manifest["BuildId"], "modules": modules, "hot_reload_modules": numbered})
}

// ---------------------------------------------------------------- the Remote Control port

/// An `http://` URL's `host[:port]` (as the Host header carries it), its host name and its port,
/// 80 when it names none. Requests and the launch and quit port checks all read the URL through
/// this, so they never wait on one port and talk to another (RA-294).
pub fn remote_addr(url: &str) -> Option<(String, String, u16)> {
    let rest = url.trim().strip_prefix("http://")?;
    let host = rest.split('/').next().unwrap_or(rest);
    let (name, port) = match host.rsplit_once(':') {
        // `[::1]` alone: the colons are the address's own.
        Some((name, port)) if !port.ends_with(']') => (name, port.parse().ok()?),
        _ => (host, 80),
    };
    Some((host.to_string(), name.trim_start_matches('[').trim_end_matches(']').to_string(), port))
}

pub fn port_of(url: &str) -> u16 {
    remote_addr(url).map(|(_, _, port)| port).unwrap_or(30010)
}

/// Whether nothing is listening on (or still holding) the port.
pub fn port_free(port: u16) -> bool {
    std::net::TcpListener::bind(("127.0.0.1", port)).is_ok()
}

pub fn wait_port_free(port: u16, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if port_free(port) {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// Log lines saying the web server could not bind: the editor is up but unreachable, and
/// `WebControl.StartServer` alone does nothing until `WebControl.StopServer` runs first.
pub fn bind_failures(log_tail: &[String]) -> Vec<String> {
    let re = regex::Regex::new(r"(?i)(unable to bind|failed to bind|could not bind|address already in use|bind.*failed).*").unwrap();
    log_tail.iter().filter(|l| re.is_match(l) && (l.contains("Http") || l.contains("WebControl") || l.contains("RemoteControl") || l.contains("Socket"))).cloned().collect()
}

/// Start the editor on the project, detached, once the port is free.
pub fn launch(engine: &Path, uproject: &Path, port: u16, extra: &[String]) -> Result<u32> {
    let binary = editor_binary(engine);
    anyhow::ensure!(binary.is_file(), "{} does not exist (build the editor first)", binary.display());
    if !wait_port_free(port, Duration::from_secs(90)) {
        bail!("port {port} is still held (a previous editor, or another program). Wait, or free it, before launching: the new editor could not start its Remote Control server");
    }
    let mut command = launch_command(&binary, uproject, extra);
    let mut child = command.spawn().with_context(|| format!("starting {}", binary.display()))?;
    let pid = child.id();
    // Reaped when it exits, however long that is: a dropped Child is never waited on, and each
    // editor would stay a zombie under this server until it quits (RA-295).
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(pid)
}

fn launch_command(binary: &Path, uproject: &Path, extra: &[String]) -> std::process::Command {
    let mut command = std::process::Command::new(binary);
    command
        // Canonical, so editors_for recognises this editor by path whatever the cwd was.
        .arg(uproject.canonicalize().unwrap_or_else(|_| uproject.to_path_buf()))
        // Starts the Remote Control web server even when auto-start is off in the project.
        .arg("-RCWebControlEnable")
        .args(extra)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    // The editor belongs to the person, not to the agent that started it. Carrying the agent's
    // RELAY_SESSION/INSTANCE/STORE would make Relay's crash recovery reap it as that agent's
    // orphan on the next engine start, and its git commits would run as the agent.
    strip_relay_identity(&mut command, std::env::vars_os().map(|(key, _)| key));
    #[cfg(unix)]
    {
        // Its own process group: the editor outlives this MCP server and the agent.
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command
}

fn strip_relay_identity(command: &mut std::process::Command, keys: impl IntoIterator<Item = std::ffi::OsString>) {
    for key in keys {
        if key.to_string_lossy().starts_with("RELAY_") {
            command.env_remove(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_this_exact_project_file_matches() {
        let root = tempfile::tempdir().unwrap();
        let main = root.path().join("Game");
        let worktree = main.join(".relay/worktrees/w1");
        std::fs::create_dir_all(&worktree).unwrap();
        for file in [main.join("Game.uproject"), main.join("TopDownGame.uproject"), worktree.join("Game.uproject")] {
            std::fs::write(file, "{}").unwrap();
        }
        let want = main.join("Game.uproject").canonicalize().unwrap();
        let line = |args: &[&str]| -> Vec<String> { std::iter::once("UnrealEditor").chain(args.iter().copied()).map(str::to_string).collect() };
        let hits = |args: &[&str], pwd: Option<&Path>| project_args(&line(args), pwd).contains(&want);
        assert!(hits(&[main.join("Game.uproject").to_str().unwrap(), "-RCWebControlEnable"], None));
        assert!(hits(&["Game.uproject"], Some(&main)), "a relative path resolves against the launch PWD");
        assert!(!hits(&["Game.uproject"], None), "an unresolvable relative path matches nothing");
        assert!(!hits(&[worktree.join("Game.uproject").to_str().unwrap()], None), "another checkout's copy");
        assert!(!hits(&[main.join("TopDownGame.uproject").to_str().unwrap()], None), "a name that ends the same way");
        assert!(!hits(&["Game.uproject"], Some(&worktree)));
    }

    #[test]
    fn the_editor_does_not_inherit_the_agents_identity() {
        let mut command = std::process::Command::new("UnrealEditor");
        let keys = ["RELAY_SESSION", "RELAY_TOKEN", "RELAY_INSTANCE", "RELAY_STORE", "PATH", "HOME"];
        strip_relay_identity(&mut command, keys.iter().map(std::ffi::OsString::from));
        let mut removed: Vec<String> = command.get_envs().filter(|(_, v)| v.is_none()).map(|(k, _)| k.to_string_lossy().into_owned()).collect();
        removed.sort();
        assert_eq!(removed, ["RELAY_INSTANCE", "RELAY_SESSION", "RELAY_STORE", "RELAY_TOKEN"]);
        // And launch really applies it to whatever this process carries.
        let launch = launch_command(Path::new("/opt/UE/UnrealEditor"), Path::new("/no/such/Game.uproject"), &[]);
        let inherited = std::env::vars_os().filter(|(k, _)| k.to_string_lossy().starts_with("RELAY_")).count();
        assert_eq!(launch.get_envs().filter(|(_, v)| v.is_none()).count(), inherited);
    }

    #[test]
    fn hot_reload_modules_are_flagged() {
        let root = tempfile::tempdir().unwrap();
        let bin = root.path().join("Binaries/Linux");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("UnrealEditor.modules"), r#"{"BuildId":"abc","Modules":{"Shooter":"libUnrealEditor-Shooter-0003.so","ShooterUI":"libUnrealEditor-ShooterUI.so"}}"#).unwrap();
        let m = module_manifest(root.path(), "Linux");
        assert_eq!(m["hot_reload_modules"], json!(["Shooter -> libUnrealEditor-Shooter-0003.so"]));
        assert_eq!(module_manifest(root.path(), "Win64")["found"], false);
    }

    #[test]
    fn bind_failures_are_found_in_the_log() {
        let lines = vec![
            "LogHttpListener: Error: HttpListener unable to bind to 127.0.0.1:30010".to_string(),
            "LogTemp: Display: unable to bind a key".to_string(),
            "LogInit: Display: Engine started".to_string(),
        ];
        assert_eq!(bind_failures(&lines).len(), 1);
    }

    #[test]
    fn a_held_port_is_not_free() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(!port_free(port));
        drop(listener);
        assert!(wait_port_free(port, Duration::from_secs(5)));
        assert_eq!(port_of("http://127.0.0.1:30010"), 30010);
        assert_eq!(port_of("http://localhost:31000/"), 31000);
        // The same reading of the URL as the requests themselves.
        assert_eq!(port_of("http://127.0.0.1:31000/remote"), 31000);
        assert_eq!(port_of("http://localhost"), 80);
        assert_eq!(port_of("not a url"), 30010);
        assert_eq!(remote_addr("http://[::1]:31000/"), Some(("[::1]:31000".into(), "::1".into(), 31000)));
        assert_eq!(remote_addr("http://[::1]"), Some(("[::1]".into(), "::1".into(), 80)));
    }

    #[test]
    fn only_this_projects_crash_reporters_are_its_own() {
        let root = tempfile::tempdir().unwrap();
        let game = root.path().join("Game");
        std::fs::create_dir_all(&game).unwrap();
        let uproject = game.join("Game.uproject");
        let reporter = |args: &[String]| Proc {
            pid: 1,
            exe: PathBuf::from("/opt/UE/Engine/Binaries/Linux/CrashReportClientEditor"),
            cmdline: std::iter::once("CrashReportClientEditor".to_string()).chain(args.iter().cloned()).collect(),
        };
        let crash = format!("{}/Saved/Crashes/crashinfo-Game-pid-42/", game.display());
        assert!(reports_for(&reporter(&[crash, "-Unattended".into()]), &uproject));
        let canonical = format!("\"{}/Saved/Crashes/x\"", game.canonicalize().unwrap().display());
        assert!(reports_for(&reporter(&[canonical]), &uproject));
        let other = format!("{}Other/Saved/Crashes/crashinfo-GameOther-pid-7/", game.display());
        assert!(!reports_for(&reporter(&[other]), &uproject), "a folder whose name merely starts the same way");
        assert!(!reports_for(&reporter(&["-Unattended".into()]), &uproject), "a reporter that names no project is not ours");
    }

    #[test]
    fn the_engine_is_found_by_version_when_registered_under_a_guid() {
        let home = tempfile::tempdir().unwrap();
        let engine = home.path().join("dev/UnrealEngine");
        std::fs::create_dir_all(engine.join("Engine/Build/BatchFiles")).unwrap();
        std::fs::write(engine.join("Engine/Build/Build.version"), r#"{"MajorVersion":5,"MinorVersion":8,"PatchVersion":0}"#).unwrap();
        let ini = home.path().join(".config/Epic/UnrealEngine");
        std::fs::create_dir_all(&ini).unwrap();
        std::fs::write(ini.join("Install.ini"), format!("[Installations]\n{{0A1B2C3D-0000}}={}\n", engine.display())).unwrap();
        let project = home.path().join("Game");
        std::fs::create_dir_all(&project).unwrap();
        // SAFETY: the only test in this binary that writes HOME or UE_ROOT. HOME is put back
        // before `home` is deleted: the Blender tests here read it too, and one that ran after
        // this used to recreate `<deleted temp dir>/.cache/relay/blender-sandbox` (11 MB, left in
        // /tmp for good, one per test run).
        let saved_home = std::env::var_os("HOME");
        unsafe {
            std::env::set_var("HOME", home.path());
            std::env::remove_var("UE_ROOT");
        }
        struct RestoreHome(Option<std::ffi::OsString>);
        impl Drop for RestoreHome {
            fn drop(&mut self) {
                // SAFETY: as above.
                unsafe {
                    match self.0.take() {
                        Some(home) => std::env::set_var("HOME", home),
                        None => std::env::remove_var("HOME"),
                    }
                }
            }
        }
        let _restore = RestoreHome(saved_home);
        let log = project.join("Saved/Logs/Game.log");
        let by_guid = engine_root(&project.join("Game.uproject"), &project, "{0A1B2C3D-0000}", &log).unwrap();
        assert_eq!(by_guid, engine);
        let by_guid_without_braces = engine_root(&project.join("Game.uproject"), &project, "0a1b2c3d-0000", &log).unwrap();
        assert_eq!(by_guid_without_braces, engine);
        let by_version = engine_root(&project.join("Game.uproject"), &project, "5.8", &log).unwrap();
        assert_eq!(by_version, engine);
        let wrong_version = engine_root(&project.join("Game.uproject"), &project, "4.27", &log).unwrap_err();
        assert!(format!("{wrong_version}").contains("Install.ini entries"), "{wrong_version}");
        // The last editor session's log names the engine outright.
        std::fs::create_dir_all(log.parent().unwrap()).unwrap();
        std::fs::write(&log, format!("LogInit: Base Directory: {}/Engine/Binaries/Linux/\n", engine.display())).unwrap();
        assert_eq!(engine_root(&project.join("Game.uproject"), &project, "4.27", &log).unwrap(), engine);
    }
}
