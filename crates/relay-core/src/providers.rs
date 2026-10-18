//! Provider profiles (SPEC §4): discovery/auth plus the only place CLI arguments are shaped.

use crate::engine::IntoBus;
use relay_bus::error::BusError;
use relay_bus::types::{Provider, ProviderInfo, Role, Session};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

pub struct Discovery {
    pub info: ProviderInfo,
    pub previous_version: Option<String>,
    pub version_changed: bool,
}

pub enum Launch<'a> {
    Fresh,
    Resume { provider_ref: Option<&'a str> },
}

/// Whether `value` can be a provider's own conversation id. It becomes an argv element on
/// resume, and an agent can report it (`session.report`), so anything that a CLI could read
/// as an option — or that is not shaped like the UUID-style ids Claude and Codex issue — is
/// refused when written and ignored when read back (an older row may predate the check).
pub fn is_provider_ref(value: &str) -> bool {
    value.len() <= 128
        && value.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && value.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

pub fn role_instructions_relative(session: &str) -> String {
    format!(".relay/sessions/{session}/role-instructions.md")
}

pub const DISCOVERY_HINT: &str = "Discover operations with `bus.ops` and inspect the exact payload \
and result with `bus.schema {\"op\":\"<op>\"}`; do not guess fields. From a shell, use \
`$RELAY_BIN ops --mine` and `$RELAY_BIN schema <op>`. Relay fills your bound `project_id` and \
`session` when omitted. `session.done` always reports the current bootstrap task, so omit \
`task` and `task_id`.";

/// Every role is told the same three things, because every role needed all three: what
/// `session.bootstrap` answers, that peers are reached through the Relay mailbox rather than
/// the provider's own agent messaging, and where the brief is (D101).
const COMMON_INSTRUCTIONS: &str = "On your first turn, obtain the current assignment and queue before \
acting: call the session.bootstrap Relay MCP tool when available; otherwise run `$RELAY_BIN q \
session.bootstrap`. It also returns your live peers, every op you may call, and your \
guardrails. Coordinate with those peers through Relay `mailbox.send` — it reaches every \
provider including Codex, which your own cross-agent messaging tool does not; send \
`{\"to\":\"*\",\"text\":\"...\"}` to broadcast, and `mailbox.outbox` shows what you sent. Declare the files you are taking on with `session.claim` — it names any \
peer already holding them — and one line of what you are doing with `session.intent`. Before \
starting sizeable work, call `guardrail.explain` with structured paths, lines, and commands; \
`guardrail.check` tests one command or write. `bus.wait` blocks until something happens, so \
never poll. Android devices are shared between sessions: an `adb install`, `adb shell am …` or \
`gradlew install*` takes a lease on the device, and while a peer holds one yours is refused with \
`device.busy` naming who is using it, for what, and since when — `bus.wait` on \
`device.lease.released`, then retry, rather than retrying in a loop. `device.list` shows each \
device's holder; `device.claim` holds a device across several steps and `device.release` frees it. A new \
worktree's `target/` arrives as a copy of the project's prebuilt dependencies: a first `cargo` \
command may wait a few seconds for it, or be refused with `build.seeding` while Relay builds them \
once for every new session — `bus.wait` on `build.seed.ready`, then retry. When a guardrail refuses something you cannot progress without, do not work around \
it: call `guardrail.request` with the `kind` and `value` its hint names and a `reason`, then \
`bus.wait` for `guardrail.request_resolved` matching your `request_id` and retry once approved; \
only the user can approve it. Work on `task` first; `tasks` is the remaining ordered queue. Report each current \
task through `session.done` with `status` `completed`, `blocked` or `partial`. All builders must \
finish before the task enters review. A shared-worktree group stays on that task until every \
reviewer finishes; do not begin queued work while peers are still building or reviewing. Relay \
then advances the group FIFO and sends the next assignment through mailbox. After reporting \
completion, wait for that assignment and read `session.bootstrap`; do not repeat `session.done`. \
When you stopped short, include \
the `blockers` that stopped you. Relay responses may carry `mail.priority`; when it is nonzero \
and you are not already handling mail, finish the current atomic action, call `mailbox.list` with \
`unread_only: true`, process the priority messages, and `mailbox.ack` each one. If \
`session.bootstrap` has a current task and you actually encountered Relay workflow friction while \
doing it, immediately before `session.done` append one concise observation with `notes.append` using \
`target: \"suggestions\"`. Name what you were \
doing and what got in the way. Do not record speculative suggestions or write them mid-task; Relay \
adds the session and task identity. Your full brief is at \
$RELAY_BRIEF.";

pub fn role_instructions(role: Role) -> String {
    let specific = match role {
        Role::Builder => "You are this Relay session's builder. If session.bootstrap has no assignment, query task.list for this project before suggesting board work; if no available tasks exist, say so instead of sending the user to an empty board. Implement only work the user assigned or authorized you to take in the current worktree, verify it, and report completion through session.done. When paired with a reviewer, send it a Relay mailbox message as soon as a file is ready for review; keep working while it reviews.",
        Role::Reviewer => "You are this Relay session's reviewer. Review the paired work against the task requirements. Review file-ready messages immediately. Do not modify files or commit; send the builder exact fixes through Relay mailbox tools, then verify its correction. Your shell runs one command at a time, with no pipes, redirection or chaining, and only a single `$RELAY_BIN` call or a read-only look: `git diff`, `git log`, `git show`, `git status`, `git blame`, `cat`, `head`, `tail`, `ls`, `wc`, `grep`, `rg`.",
        Role::Docs => "You are this Relay session's documentation agent. Change only documentation required by that assignment, verify relevant documentation checks, and report completion through session.done.",
    };
    format!("{specific}\n\n{COMMON_INSTRUCTIONS}\n\n{DISCOVERY_HINT}")
}

/// How much of the brief Codex gets inline, measured as JSON-escaped bytes. The whole setting
/// is one argv element, and Linux refuses any single one over 128 KiB (`MAX_ARG_STRLEN`): a
/// brief past that made every Codex launch in the project fail with E2BIG. The rest is in the
/// brief file, which the instructions point at.
const CODEX_BRIEF_LIMIT: usize = 64 * 1024;

/// Codex takes no `--mcp-config`, so Relay's bus instructions travel through this provider-native
/// setting. Guardrail and lifecycle enforcement bind separately through project hooks (D132).
fn codex_developer_instructions(role: Role, brief: Option<&str>) -> String {
    let mut text = role_instructions(role);
    text.push_str(
        "\n\nRelay tools are not registered with this provider: reach the bus by running \
`$RELAY_BIN q <op> '<json>'` in a shell. `$RELAY_BIN q session.bootstrap` lists every op your \
role may call.",
    );
    if let Some(brief) = brief.map(str::trim).filter(|brief| !brief.is_empty()) {
        text.push_str("\n\n");
        let mut used = 0;
        let cut = brief.char_indices().find_map(|(at, ch)| {
            used += json_escaped_len(ch);
            (used > CODEX_BRIEF_LIMIT).then_some(at)
        });
        match cut {
            None => text.push_str(brief),
            Some(at) => {
                text.push_str(&brief[..at]);
                text.push_str("\n\n[The brief is cut short here. Read the whole of it at $RELAY_BRIEF.]");
            }
        }
    }
    let value = serde_json::to_string(&text).expect("Relay role instructions serialize");
    format!("developer_instructions={value}")
}

/// The bytes `ch` takes inside a JSON string.
fn json_escaped_len(ch: char) -> usize {
    match ch {
        '"' | '\\' | '\n' | '\r' | '\t' | '\u{8}' | '\u{c}' => 2,
        ch if (ch as u32) < 0x20 => 6,
        ch => ch.len_utf8(),
    }
}

pub fn codex_notify_config(relay: &Path) -> String {
    let command = [relay.display().to_string(), "hook".into(), "codex-notify".into()];
    format!("notify={}", serde_json::to_string(&command).expect("Codex notify command serializes"))
}

/// The variables a Relay launch adds to the agent's environment (see `prepare_launch`).
const RELAY_LAUNCH_ENV: &[&str] = &[
    "RELAY_SESSION", "RELAY_TOKEN", "RELAY_INSTANCE", "RELAY_PROJECT", "RELAY_WORKTREE",
    "RELAY_BRIEF", "RELAY_BIN", "RELAY_STORE",
];

/// `--config` overrides that register one plugin MCP server with Codex, which takes no MCP
/// config file (D159). Values are JSON strings and arrays, which TOML reads identically; a
/// name that is not a bare TOML key is refused rather than quoted into a different key.
///
/// Codex starts an MCP server with a scrubbed environment (PATH, HOME and a few more) plus the
/// names in its `env_vars`. Claude passes its whole environment on, and the plugins are written
/// for that (`UE_ROOT`, `RELAY_WORKTREE`, …), so the server is given the same here: every
/// variable of the engine's environment, which the agent inherits, and Relay's launch variables.
pub fn codex_mcp_config(
    name: &str,
    command: &str,
    args: &[String],
    env: &std::collections::BTreeMap<String, String>,
) -> Vec<String> {
    let mut forwarded: Vec<String> = std::env::vars_os()
        .filter_map(|(key, _)| key.into_string().ok())
        .chain(RELAY_LAUNCH_ENV.iter().map(|key| key.to_string()))
        .filter(|key| !env.contains_key(key))
        .collect();
    forwarded.sort();
    forwarded.dedup();
    codex_mcp_config_with(name, command, args, env, &forwarded)
}

fn codex_mcp_config_with(
    name: &str,
    command: &str,
    args: &[String],
    env: &std::collections::BTreeMap<String, String>,
    forwarded: &[String],
) -> Vec<String> {
    let bare = |key: &str| !key.is_empty() && key.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-');
    if !bare(name) || !env.keys().all(|key| bare(key)) {
        return Vec::new();
    }
    let quote = |text: &str| serde_json::to_string(text).expect("a string serializes");
    let mut out = vec![
        "--config".into(),
        format!("mcp_servers.{name}.command={}", quote(command)),
        "--config".into(),
        format!("mcp_servers.{name}.args={}", serde_json::to_string(args).expect("strings serialize")),
    ];
    if !env.is_empty() {
        let pairs = env.iter().map(|(key, value)| format!("{key}={}", quote(value))).collect::<Vec<_>>().join(",");
        out.extend(["--config".into(), format!("mcp_servers.{name}.env={{{pairs}}}")]);
    }
    if !forwarded.is_empty() {
        out.extend([
            "--config".into(),
            format!("mcp_servers.{name}.env_vars={}", serde_json::to_string(forwarded).expect("strings serialize")),
        ]);
    }
    out
}

pub trait Driver: Sync {
    fn binary(&self) -> &'static str;
    /// `brief` is the compact session brief. Providers that read a prompt file ignore it;
    /// providers with no such channel inline it.
    fn args(&self, session: &Session, launch: Launch<'_>, brief: Option<&str>) -> Vec<String>;
    /// Whether Relay's `PreToolUse` guardrail and lifecycle reports actually bind here.
    fn guarded(&self) -> bool;
    /// Who the CLI is signed in as: `Some(None)` when it answered and nobody is, `None` when
    /// it gave no answer at all (it timed out or would not start), which says nothing.
    fn auth(&self, path: &Path) -> Option<Option<String>>;
}

struct Claude;
struct Codex;

static CLAUDE: Claude = Claude;
static CODEX: Codex = Codex;

pub fn driver(provider: Provider) -> &'static dyn Driver {
    match provider {
        Provider::Claude => &CLAUDE,
        Provider::Codex => &CODEX,
    }
}

/// [`spawn_profile`] for `provider`, built once. Everything it reads is fixed in the build — a
/// placeholder session, the inline settings, the role texts — and `app.status`, which the shell
/// reads on connect and after changes, spent half its instructions rebuilding both providers'.
fn spawn_profile_of(provider: Provider) -> Value {
    static PROFILES: std::sync::LazyLock<[Value; 2]> =
        std::sync::LazyLock::new(|| [spawn_profile(driver(Provider::Claude)), spawn_profile(driver(Provider::Codex))]);
    PROFILES[match provider { Provider::Claude => 0, Provider::Codex => 1 }].clone()
}

/// `provider.list`'s `spawn_profile`: the command lines [`Driver::args`] builds, for a
/// placeholder session that sets every option, so it cannot drift from what is run (RA-617).
/// An argument too long to read (inline settings, role instructions) is shown by its name.
fn spawn_profile(drv: &dyn Driver) -> Value {
    let session = Session {
        id: 0, name: "<session>".into(), intent: None, project_id: 0, provider: Provider::Claude, role: Role::Builder,
        model: Some("<model>".into()), effort: Some("<effort>".into()), branch: String::new(), worktree: String::new(),
        task_id: None, module_id: None, pair_with: None, bus_writes: false, allow_ui: false, use_api_key: false, state: relay_bus::types::SessionState::Created,
        pid: None, exit_code: None, provider_ref: None, spawned_at: None, last_output_at: None, usage: None,
        created_at: String::new(), updated_at: String::new(), closed_at: None,
    };
    let shape = |launch| -> Vec<String> {
        drv.args(&session, launch, None).into_iter().map(|arg| {
            if arg.len() <= 120 { return arg; }
            match arg.split_once('=') {
                Some((key, _)) if key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.') => format!("{key}=<{key}>"),
                _ => "<inline JSON>".into(),
            }
        }).collect()
    };
    json!({
        "binary": drv.binary(), "fresh": shape(Launch::Fresh),
        "resume": shape(Launch::Resume { provider_ref: Some("<provider_ref>") }),
        "fallback_resume": shape(Launch::Resume { provider_ref: None }), "guarded": drv.guarded(),
    })
}

fn model_effort_args(session: &Session, claude: bool) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(model) = &session.model {
        args.extend(["--model".to_string(), model.clone()]);
    }
    if let Some(effort) = &session.effort {
        if claude {
            args.extend(["--effort".to_string(), effort.clone()]);
        } else {
            args.extend(["--config".to_string(), format!("model_reasoning_effort=\"{effort}\"")]);
        }
    }
    args
}

impl Driver for Claude {
    fn binary(&self) -> &'static str { "claude" }
    fn guarded(&self) -> bool { true }
    fn args(&self, session: &Session, launch: Launch<'_>, _brief: Option<&str>) -> Vec<String> {
        let mut args = vec![
            "--mcp-config".into(), crate::hooks::MCP_CONFIG_RELATIVE.into(),
            "--settings".into(), crate::hooks::claude_settings(session.use_api_key.then(crate::hooks::api_key_helper).as_deref()),
            "--append-system-prompt-file".into(), role_instructions_relative(&session.name),
        ];
        match launch {
            Launch::Fresh => {
                args.extend(["--name".into(), session.name.clone()]);
                args.extend(model_effort_args(session, true));
            }
            Launch::Resume { provider_ref } => {
                match provider_ref {
                    Some(reference) => args.extend(["--resume".into(), reference.to_string()]),
                    None => args.push("--continue".into()),
                }
                args.extend(model_effort_args(session, true));
            }
        }
        args
    }
    fn auth(&self, path: &Path) -> Option<Option<String>> {
        let output = command_output(path, &["auth", "status"])?;
        if !output.success { return Some(None); }
        let Ok(value) = serde_json::from_str::<Value>(&output.stdout) else { return Some(None) };
        if value["loggedIn"].as_bool() != Some(true) { return Some(None); }
        Some(value["email"].as_str().or_else(|| value["orgName"].as_str()).map(str::to_string))
    }
}

impl Driver for Codex {
    fn binary(&self) -> &'static str { "codex" }
    fn guarded(&self) -> bool { true }
    fn args(&self, session: &Session, launch: Launch<'_>, brief: Option<&str>) -> Vec<String> {
        let mut args = Vec::new();
        match launch {
            Launch::Fresh => {
                args.push("--no-alt-screen".into());
                args.push("--approve-for-me".into());
                args.extend(model_effort_args(session, false));
                args.extend(["--config".into(), codex_developer_instructions(session.role, brief)]);
            }
            Launch::Resume { provider_ref } => {
                args.push("resume".into());
                args.push("--no-alt-screen".into());
                args.push("--approve-for-me".into());
                args.extend(model_effort_args(session, false));
                args.extend(["--config".into(), codex_developer_instructions(session.role, brief)]);
                match provider_ref {
                    Some(reference) => args.push(reference.to_string()),
                    None => args.push("--last".into()),
                }
            }
        }
        args
    }
    fn auth(&self, path: &Path) -> Option<Option<String>> {
        let output = command_output(path, &["login", "status"])?;
        if !output.success { return Some(None); }
        let line = output.stdout.lines().find(|line| !line.trim().is_empty()).map(str::trim);
        Some(line.map(|line| line.strip_prefix("Logged in using ").unwrap_or(line).trim().to_string()))
    }
}

pub fn validate_options(provider: Provider, model: Option<&str>, effort: Option<&str>) -> Result<(), BusError> {
    if model.is_some_and(|model| model.trim().is_empty()) {
        return Err(BusError::invalid("session.model", "model cannot be empty"));
    }
    let allowed: &[&str] = match provider {
        Provider::Claude => &["low", "medium", "high", "xhigh", "max"],
        Provider::Codex => &["minimal", "low", "medium", "high", "xhigh"],
    };
    if let Some(effort) = effort {
        if !allowed.contains(&effort) {
            return Err(BusError::invalid(
                "session.effort",
                format!("{} effort {effort:?} must be one of {}", driver(provider).binary(), allowed.join(", ")),
            ));
        }
    }
    Ok(())
}

fn configured_path(conn: &Connection, provider: Provider) -> Option<PathBuf> {
    let key = format!("providers.{}.path", driver(provider).binary());
    let configured: Option<String> = conn.prepare_cached("SELECT value FROM settings WHERE path=?1").ok()?
        .query_row([key], |row| row.get(0)).optional().ok().flatten();
    configured.and_then(|raw| serde_json::from_str::<Option<String>>(&raw).ok().flatten()).map(PathBuf::from)
}

pub fn executable(conn: &Connection, provider: Provider) -> Result<PathBuf, BusError> {
    let drv = driver(provider);
    if let Some(path) = configured_path(conn, provider) {
        if path.is_file() { return Ok(path); }
        return Err(BusError::unavailable(
            "provider.not_installed",
            format!("providers.{}.path = {:?} does not exist", drv.binary(), path.display().to_string()),
        ));
    }
    which::which(drv.binary()).map_err(|_| {
        BusError::unavailable("provider.not_installed", format!("{} is not on PATH", drv.binary()))
            .with_hint(format!("install it, or set settings providers.{}.path", drv.binary()))
    })
}

type CacheRow = (Option<String>, Option<String>, Option<String>, Option<String>);

fn cache_row(row: &Row) -> rusqlite::Result<CacheRow> {
    Ok((row.get("path")?, row.get("version")?, row.get("signed_in_as")?, row.get("last_seen_version")?))
}

pub fn list(conn: &Connection) -> Result<Vec<ProviderInfo>, BusError> {
    [Provider::Claude, Provider::Codex].into_iter().map(|provider| {
        let drv = driver(provider);
        let resolved = executable(conn, provider).ok();
        let cached = conn.prepare_cached("SELECT * FROM provider_cache WHERE provider=?1").bus()?
            .query_row([drv.binary()], cache_row).optional().bus()?;
        let (path, version, signed_in_as, last_seen_version) = cached.unwrap_or_default();
        let path = resolved.as_ref().map(|p| p.display().to_string()).or(path);
        // Derived, not the cached copy: what this build runs, not what the last refresh's did.
        Ok(ProviderInfo { provider, installed: resolved.is_some(), path, version, signed_in_as, last_seen_version, spawn_profile: spawn_profile_of(provider), guarded: drv.guarded() })
    }).collect()
}

/// Probe and record in one step, for a caller that already holds a transaction.
pub fn refresh(tx: &Transaction, now: &str) -> Result<Vec<Discovery>, BusError> {
    record(tx, now, probe(paths(tx)))
}

/// Where each provider's binary is, read under the store lock: the input to [`probe`].
pub fn paths(conn: &Connection) -> Vec<(Provider, Option<PathBuf>)> {
    [Provider::Claude, Provider::Codex].into_iter().map(|provider| (provider, executable(conn, provider).ok())).collect()
}

/// What one provider's binary said about itself, gathered with no lock held. With a `path`,
/// a `None` version or sign-in is no answer (the probe timed out or failed), not an absence:
/// [`record`] keeps what it knew rather than report the provider gone (RA-328).
pub struct Probe {
    provider: Provider,
    path: Option<PathBuf>,
    version: Option<String>,
    signed_in_as: Option<Option<String>>,
}

/// Run every provider's `--version` and auth probe, in parallel. These are subprocesses, so
/// `provider.refresh` runs them before its transaction opens (D149): each one may take up to
/// [`PROBE_TIMEOUT`], and the store mutex is not where that wait belongs.
pub fn probe(paths: Vec<(Provider, Option<PathBuf>)>) -> Vec<Probe> {
    thread::scope(|scope| {
        paths.into_iter().map(|(provider, path)| scope.spawn(move || {
            let drv = driver(provider);
            let (version, signed_in_as) = thread::scope(|scope| {
                let version = scope.spawn(|| path.as_deref().and_then(|path| command_output(path, &["--version"]))
                    .filter(|output| output.success).map(|output| first_line(&output.stdout)));
                let auth = scope.spawn(|| match path.as_deref() { Some(path) => drv.auth(path), None => Some(None) });
                (version.join().expect("provider version probe panicked"), auth.join().expect("provider auth probe panicked"))
            });
            Probe { provider, path, version, signed_in_as }
        })).collect::<Vec<_>>().into_iter().map(|handle| handle.join().expect("provider probe panicked")).collect::<Vec<_>>()
    })
}

/// Store what [`probe`] found, comparing against the cached version inside `tx`.
pub fn record(tx: &Transaction, now: &str, probes: Vec<Probe>) -> Result<Vec<Discovery>, BusError> {
    let mut out = Vec::new();
    for Probe { provider, path, version, signed_in_as } in probes {
        let drv = driver(provider);
        let old: Option<CacheRow> = tx.query_row(
            "SELECT * FROM provider_cache WHERE provider=?1", [drv.binary()], cache_row,
        ).optional().bus()?;
        let (old_path, previous_version, old_signed_in, old_last_seen) = old.unwrap_or_default();
        // A binary that did not answer is still the binary it was: a slow cold start must not
        // read as "not installed", nor sign the account out (RA-328). Only a vanished
        // executable, or a different one, clears what was known.
        let same_binary = path.is_some() && old_path == path.as_ref().map(|p| p.display().to_string());
        let version = if version.is_none() && same_binary { previous_version.clone() } else { version };
        let signed_in_as = match signed_in_as {
            Some(answer) => answer,
            None if same_binary => old_signed_in,
            None => None,
        };
        let version_changed = previous_version.is_some() && previous_version != version;
        let last_seen_version = if version_changed { previous_version.clone() } else { old_last_seen };
        let profile = spawn_profile_of(provider);
        tx.execute(
            "INSERT INTO provider_cache(provider,path,version,signed_in_as,last_seen_version,spawn_profile,detected_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7)
             ON CONFLICT(provider) DO UPDATE SET path=excluded.path,version=excluded.version,
               signed_in_as=excluded.signed_in_as,last_seen_version=excluded.last_seen_version,
               spawn_profile=excluded.spawn_profile,detected_at=excluded.detected_at",
            params![drv.binary(), path.as_ref().map(|p| p.display().to_string()), version, signed_in_as, last_seen_version, serde_json::to_string(&profile).bus()?, now],
        ).bus()?;
        out.push(Discovery {
            info: ProviderInfo { provider, installed: path.is_some(), path: path.map(|p| p.display().to_string()), version, signed_in_as, last_seen_version, spawn_profile: profile, guarded: drv.guarded() },
            previous_version,
            version_changed,
        });
    }
    Ok(out)
}

struct Output { success: bool, stdout: String }

/// Provider probes (`<binary> --version`) are bounded so a wedged binary cannot hang a refresh, or
/// the store mutex where a caller still probes under it (`provider.update`'s follow-up, D144).
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

fn command_output(path: &Path, args: &[&str]) -> Option<Output> {
    let mut command = Command::new(path);
    command.args(args);
    // `None` for a probe that would not start or was killed at the deadline: no answer, which
    // the caller must not read as "not installed" or "signed out".
    let output = crate::proc::output_with_timeout(&mut command, PROBE_TIMEOUT).ok()??;
    let stdout = if output.stdout.is_empty() {
        String::from_utf8_lossy(&output.stderr).to_string()
    } else {
        String::from_utf8_lossy(&output.stdout).to_string()
    };
    Some(Output { success: output.status.success(), stdout })
}

fn first_line(raw: &str) -> String {
    raw.lines().find(|line| !line.trim().is_empty()).unwrap_or_default().trim().chars().take(200).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use relay_bus::types::{Role, SessionState};

    fn session(provider: Provider) -> Session {
        Session { id: 1, name: "calm-otter".into(), intent: None, project_id: 1, provider, role: Role::Builder,
            model: Some("model-x".into()), effort: Some("high".into()), branch: "relay/calm-otter".into(), worktree: "/tmp/w".into(),
            task_id: None, module_id: None, pair_with: None, bus_writes: false, allow_ui: false, use_api_key: false, state: SessionState::Created,
            pid: None, exit_code: None, provider_ref: None, spawned_at: None, last_output_at: None, usage: None,
            created_at: "now".into(), updated_at: "now".into(), closed_at: None }
    }

    #[test]
    fn provider_refs_cannot_carry_options() {
        assert!(is_provider_ref("0f6e1c2a-4b1d-4c55-9a51-3b8f8c1d2e3f"));
        assert!(is_provider_ref("cx_id-2"));
        for bad in ["", "--dangerously-skip-permissions", "-x", "--config=sandbox_mode=\"danger-full-access\"",
            "id with space", "id/../x", "id=1", &"a".repeat(129)] {
            assert!(!is_provider_ref(bad), "{bad:?} must be refused");
        }
    }

    #[test]
    fn profiles_shape_fresh_and_resume_commands() {
        assert!(role_instructions(Role::Builder).contains("query task.list"));
        assert_eq!(codex_notify_config(Path::new("/opt/Relay 2/relay")), "notify=[\"/opt/Relay 2/relay\",\"hook\",\"codex-notify\"]");
        let claude = session(Provider::Claude);
        let settings = crate::hooks::claude_settings(None);
        assert_eq!(driver(Provider::Claude).args(&claude, Launch::Fresh, None),
            ["--mcp-config", ".relay/relay.mcp.json", "--settings", settings.as_str(), "--append-system-prompt-file", ".relay/sessions/calm-otter/role-instructions.md", "--name", "calm-otter", "--model", "model-x", "--effort", "high"]);
        assert_eq!(driver(Provider::Claude).args(&claude, Launch::Resume { provider_ref: Some("cc-id") }, None),
            ["--mcp-config", ".relay/relay.mcp.json", "--settings", settings.as_str(), "--append-system-prompt-file", ".relay/sessions/calm-otter/role-instructions.md", "--resume", "cc-id", "--model", "model-x", "--effort", "high"]);
        // An API-key session differs only in its settings, on resume as on a fresh launch.
        let mut keyed = claude.clone();
        keyed.use_api_key = true;
        let keyed_settings = crate::hooks::claude_settings(Some(&crate::hooks::api_key_helper()));
        for launch in [Launch::Fresh, Launch::Resume { provider_ref: Some("cc-id") }] {
            let args = driver(Provider::Claude).args(&keyed, launch, None);
            assert_eq!(args[3], keyed_settings);
        }
        let mut codex = session(Provider::Codex);
        codex.role = Role::Reviewer;
        let args = driver(Provider::Codex).args(&codex, Launch::Resume { provider_ref: Some("cx-id") }, None);
        assert_eq!(&args[..7], ["resume", "--no-alt-screen", "--approve-for-me", "--model", "model-x", "--config", "model_reasoning_effort=\"high\""]);
        assert_eq!(args[7], "--config");
        assert!(args[8].starts_with("developer_instructions=\"You are this Relay session's reviewer."));
        assert_eq!(args[9], "cx-id");
    }

    /// `app.status` and `provider.list` answer with the profile built once; it is the one built now.
    #[test]
    fn the_kept_spawn_profile_is_the_one_built_now() {
        for provider in [Provider::Claude, Provider::Codex] {
            assert_eq!(spawn_profile_of(provider), spawn_profile(driver(provider)));
        }
    }

    #[test]
    fn the_spawn_profile_is_the_command_line_args_builds() {
        let claude = spawn_profile(driver(Provider::Claude));
        let resume: Vec<&str> = claude["resume"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        assert_eq!(resume[..4], ["--mcp-config", ".relay/relay.mcp.json", "--settings", "<inline JSON>"]);
        assert!(resume.ends_with(&["--resume", "<provider_ref>", "--model", "<model>", "--effort", "<effort>"]), "{resume:?}");
        let codex = spawn_profile(driver(Provider::Codex));
        let fallback: Vec<&str> = codex["fallback_resume"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        assert!(fallback.contains(&"model_reasoning_effort=\"<effort>\"") && fallback.ends_with(&["--last"]), "{fallback:?}");
        assert!(fallback.contains(&"developer_instructions=<developer_instructions>"), "{fallback:?}");
    }

    #[test]
    fn a_probe_that_gives_no_answer_keeps_what_was_known() {
        let store = crate::Store::open_memory().unwrap();
        let mut conn = store.lock();
        let tx = conn.transaction().unwrap();
        let path = Some(PathBuf::from("/opt/claude"));
        let probe = |path: Option<PathBuf>, version: Option<&str>, signed_in_as: Option<Option<&str>>| Probe {
            provider: Provider::Claude, path, version: version.map(str::to_string),
            signed_in_as: signed_in_as.map(|answer| answer.map(str::to_string)),
        };
        record(&tx, "t0", vec![probe(path.clone(), Some("claude 1.0"), Some(Some("me@example.test")))]).unwrap();
        // Timed out: the version and the account stand, and nothing changed.
        let slow = record(&tx, "t1", vec![probe(path.clone(), None, None)]).unwrap().remove(0);
        assert!(!slow.version_changed);
        assert_eq!(slow.info.version.as_deref(), Some("claude 1.0"));
        assert_eq!(slow.info.signed_in_as.as_deref(), Some("me@example.test"));
        // An answer that nobody is signed in is believed.
        let out = record(&tx, "t2", vec![probe(path.clone(), Some("claude 1.0"), Some(None))]).unwrap().remove(0);
        assert_eq!(out.info.signed_in_as, None);
        // The executable vanished: that is a change.
        let gone = record(&tx, "t3", vec![probe(None, None, Some(None))]).unwrap().remove(0);
        assert!(gone.version_changed && gone.info.version.is_none());
    }

    #[test]
    fn every_role_is_told_about_the_brief_and_the_mailbox() {
        for role in [Role::Builder, Role::Reviewer, Role::Docs] {
            let text = role_instructions(role);
            assert!(text.contains("$RELAY_BRIEF"), "{role:?} is never told where its brief is");
            assert!(text.contains("mailbox.send"), "{role:?} is never told the mailbox exists");
            assert!(text.contains("{\"to\":\"*\",\"text\":\"...\"}"), "{role:?} is never told the broadcast alias");
            assert!(text.contains("guardrail.check"), "{role:?} cannot find the dry run");
            assert!(text.contains("paths, lines, and commands"), "{role:?} is invited to send prose to guardrail.explain");
            assert!(text.contains("session.claim"), "{role:?} is never told it can claim files");
            assert!(text.contains("bus.schema"), "{role:?} is left to guess payload fields");
            assert!(text.contains("$RELAY_BIN schema <op>"), "{role:?} cannot inspect schemas from its shell");
            assert!(text.contains("omit `task` and `task_id`"), "{role:?} may guess a task field for session.done");
            assert!(text.contains("bus.wait"), "{role:?} is left to poll");
            assert!(text.contains("blockers"), "{role:?} can only report success");
            assert!(text.contains("mail.priority"), "{role:?} will miss priority mail");
            assert!(text.contains("has a current task"), "{role:?} will try to file an unattributed suggestion");
            assert!(text.contains("target: \"suggestions\""), "{role:?} cannot record observed friction");
        }
        // The common half is one string; keep the roles from drifting apart again.
        assert!(COMMON_INSTRUCTIONS.contains("mailbox.send"));
        for role in [Role::Builder, Role::Reviewer, Role::Docs] {
            assert!(role_instructions(role).contains(COMMON_INSTRUCTIONS));
        }
    }

    #[test]
    fn codex_carries_the_brief_and_the_shell_path_it_actually_has() {
        let codex = session(Provider::Codex);
        let args = driver(Provider::Codex).args(&codex, Launch::Fresh, Some("## Live peers\n- sly-egret"));
        assert_eq!(&args[..2], ["--no-alt-screen", "--approve-for-me"]);
        let config = args.iter().find(|arg| arg.starts_with("developer_instructions=")).expect("developer_instructions");
        assert!(config.contains("sly-egret"), "codex never receives its peers");
        assert!(config.contains("$RELAY_BIN q"), "codex is not told how to reach the bus");
        assert!(driver(Provider::Codex).guarded());
        assert!(driver(Provider::Claude).guarded());
    }

    #[test]
    fn a_huge_brief_is_cut_to_fit_one_argv_element() {
        let codex = session(Provider::Codex);
        // Quotes and newlines double in JSON; the cap is measured after escaping.
        let brief = "## Live peers\n- sly-egret \"quoted\"\n".repeat(20_000);
        let args = driver(Provider::Codex).args(&codex, Launch::Fresh, Some(&brief));
        let config = args.iter().find(|arg| arg.starts_with("developer_instructions=")).unwrap();
        assert!(config.len() < 128 * 1024, "{} bytes cannot be exec'd", config.len());
        assert!(config.contains("sly-egret"));
        assert!(config.contains("Read the whole of it at $RELAY_BRIEF"));
        let short = driver(Provider::Codex).args(&codex, Launch::Fresh, Some("- sly-egret"));
        assert!(!short.iter().any(|arg| arg.contains("cut short")));
    }

    #[test]
    fn codex_mcp_servers_get_the_environment_claude_would_give_them() {
        let env = std::collections::BTreeMap::from([("UE_ROOT".to_string(), "/opt/ue".to_string())]);
        let args = codex_mcp_config_with("unreal", "/bin/relay", &["unreal-mcp".into()], &env,
            &["RELAY_WORKTREE".into(), "UE_PROJECT".into()]);
        assert!(args.contains(&"mcp_servers.unreal.env={UE_ROOT=\"/opt/ue\"}".to_string()), "{args:?}");
        assert!(args.contains(&"mcp_servers.unreal.env_vars=[\"RELAY_WORKTREE\",\"UE_PROJECT\"]".to_string()), "{args:?}");
        let live = codex_mcp_config("unreal", "/bin/relay", &[], &env).join(" ");
        assert!(live.contains("\"RELAY_SESSION\"") && live.contains("\"RELAY_TOKEN\""), "{live}");
        assert!(!live.contains("\"UE_ROOT\""), "a fixed value is not also forwarded: {live}");
    }
}
