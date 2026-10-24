//! Provider and VCS enforcement adapters (BUS.md §9.3). These files contain no policy:
//! they translate the host hook into a `guardrail.gate` call on the same bus as every other
//! client.

// Every file read here sits where an agent can write — its checkout, `<repo>/.relay/hooks` —
// and is read under `writes()` or, by the launch sweep, under the store mutex; a FIFO planted
// there would block every launch, close and commit, or Relay's start.
use crate::private::read_regular;
use crate::Instance;
use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const CLAUDE_HOOK_MARKER: &str = "hook claude-";
const CLAUDE_PRE_TOOL: &str = "hook claude-pre-tool";
const CODEX_HOOK_MARKER: &str = "hook codex-";
const CODEX_PRE_TOOL: &str = "hook codex-pre-tool";
const CODEX_HOOKS_RELATIVE: &str = ".codex/hooks.json";
const PREVIOUS_HOOKS_PATH: &str = ".relay-previous-hooks-path";
/// The instance that wrote a hook directory. Dev and stable Relay share `<repo>/.relay/hooks`,
/// and each instance's sweep must leave the other's live directories alone.
const HOOK_OWNER: &str = ".relay-instance";
pub const MCP_CONFIG_RELATIVE: &str = ".relay/relay.mcp.json";
/// Where older Relay wrote the status-line settings and script. Both sat in the agent-writable
/// checkout while Claude executed the script on every turn, outside any guardrail; the settings
/// now travel inline in argv ([`claude_settings`]) and these are only ever removed.
const LEGACY_CLAUDE_SETTINGS_RELATIVE: &str = ".relay/relay.settings.json";
const LEGACY_CLAUDE_STATUSLINE_RELATIVE: &str = ".relay/agent-statusline.sh";

/// Prefer an explicit CLI, then the CLI shipped/built beside the current executable, then PATH.
/// Desktop launchers commonly have a reduced PATH, so hooks must persist an absolute sibling path
/// instead of assuming a terminal-installed `relay` is visible.
pub fn relay_bin() -> PathBuf {
    relay_bin_from(
        std::env::var_os("RELAY_BIN").map(PathBuf::from),
        std::env::current_exe().ok(),
    )
}

fn relay_bin_from(explicit: Option<PathBuf>, current_exe: Option<PathBuf>) -> PathBuf {
    if let Some(path) = explicit.as_deref() {
        if let Ok(resolved) = which::which(path) {
            return resolved;
        }
    }
    if let Some(sibling) = current_exe
        .as_deref()
        .and_then(Path::parent)
        .map(|parent| parent.join("relay"))
        .filter(|path| is_executable(path))
    {
        return sibling;
    }
    which::which("relay")
        .ok()
        .or(explicit)
        .unwrap_or_else(|| PathBuf::from("relay"))
}

fn is_executable(path: &Path) -> bool {
    fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

/// Serializes the hook and adapter writes of launches, creates and closes, and a bus commit's
/// [`refresh_git`]. They used to be serialized by the store mutex; now that they run with it
/// released, two of them touching one repository at once would race on `.git/config`'s lock
/// file and on the adapters' temp files. Held across local `git config` calls and file writes,
/// never across a network call, and released before a launch copies skill folders, which
/// `skills::apply` serializes with its own lock.
pub fn writes() -> std::sync::MutexGuard<'static, ()> {
    static WRITES: std::sync::Mutex<()> = std::sync::Mutex::new(());
    WRITES.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The hooks git runs from `core.hooksPath` that Relay does not gate. Each gets a forwarder to
/// the user's own directory, checked when it runs, so a hook installed later (`git lfs
/// install`) still runs; without them a session checkout lost git-lfs's pre-push upload,
/// post-checkout and every other hook the moment Relay pointed `core.hooksPath` at its own dir.
const FORWARDED_HOOKS: &[&str] = &[
    "applypatch-msg", "pre-applypatch", "post-applypatch", "pre-merge-commit",
    "prepare-commit-msg", "commit-msg", "post-commit", "pre-rebase", "post-checkout",
    "post-merge", "post-rewrite", "pre-auto-gc",
];

/// The hooks Relay writes itself, each running the user's own hook of that name first:
/// pre-commit gates the commit, pre-push runs the project's check (`guardrails.check_on_push`).
const OWN_HOOKS: &[&str] = &["pre-commit", "pre-push"];

fn absolute_in(worktree: &Path, path: impl Into<PathBuf>) -> PathBuf {
    let path = path.into();
    if path.is_absolute() { path } else { worktree.join(path) }
}

/// The hooks directory git would use in `worktree` without a worktree-level `core.hooksPath`:
/// the value from any other config scope, else the repository's own `hooks`.
fn inherited_hooks_dir(worktree: &Path) -> Option<PathBuf> {
    let configured = git_optional(worktree, &["config", "--show-scope", "--get-all", "core.hooksPath"])
        .and_then(|out| out.lines()
            .rev()
            .filter_map(|line| line.split_once('\t'))
            .find(|(scope, _)| *scope != "worktree")
            .map(|(_, value)| value.to_string()));
    match configured {
        Some(path) => Some(absolute_in(worktree, path)),
        None => git_optional(worktree, &["rev-parse", "--git-common-dir"])
            .map(|dir| absolute_in(worktree, dir).join("hooks")),
    }
}

fn read_record(dir: &Path) -> Option<String> {
    read_regular(&dir.join(PREVIOUS_HOOKS_PATH))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// What a checkout had before Relay: its own worktree-level `core.hooksPath` (`config`, `None`
/// when it had none, which is what uninstalling restores) and the directory the user's hooks
/// actually live in (`chain`), never one of Relay's.
struct Previous {
    /// The checkout's worktree-level `core.hooksPath` as it stands, Relay's or not.
    current: Option<String>,
    config: Option<String>,
    chain: Option<PathBuf>,
}

fn previous_hooks(repo: &Path, worktree: &Path) -> Previous {
    let relay_hooks = repo.join(".relay").join("hooks");
    let current = git_optional(worktree, &["config", "--worktree", "--get", "core.hooksPath"]);
    let config = match current.clone() {
        // Replacing a Relay dir: what it replaced is in its record.
        Some(configured) if absolute_in(worktree, &configured).starts_with(&relay_hooks) => {
            read_record(&absolute_in(worktree, configured))
        }
        other => other,
    };
    let inherited = inherited_hooks_dir(worktree);
    // Older Relay recorded the effective directory even when the checkout had no value of its
    // own. That is the inherited one; restoring it would pin it at worktree level for good.
    let config = config.filter(|value| Some(absolute_in(worktree, value)) != inherited);
    let chain = config
        .as_ref()
        .map(|value| absolute_in(worktree, value))
        .or(inherited)
        .filter(|dir| !dir.starts_with(&relay_hooks));
    Previous { current, config, chain }
}

/// `root/relative` as a real directory, created as needed. A symlink an agent planted on the
/// way is removed and replaced, so nothing Relay writes below it lands outside `root`.
fn real_dir(root: &Path, relative: &str) -> Result<PathBuf> {
    let mut dir = root.to_path_buf();
    for part in Path::new(relative).components() {
        let std::path::Component::Normal(part) = part else {
            return Err(anyhow!("{relative} must be a plain relative path"));
        };
        dir.push(part);
        match fs::symlink_metadata(&dir) {
            Ok(meta) if meta.is_dir() => continue,
            Ok(meta) if meta.file_type().is_symlink() => {
                tracing::warn!(path = %dir.display(), "replacing a symlink in the way of a Relay file");
                fs::remove_file(&dir).with_context(|| format!("removing {}", dir.display()))?;
            }
            Ok(_) => return Err(anyhow!("{} is not a directory", dir.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).with_context(|| format!("inspecting {}", dir.display())),
        }
        match fs::create_dir(&dir) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists
                && fs::symlink_metadata(&dir).is_ok_and(|meta| meta.is_dir()) => {}
            Err(error) => return Err(error).with_context(|| format!("creating {}", dir.display())),
        }
    }
    Ok(dir)
}

/// Replace `path` through a fresh temp file beside it and a rename. The temp file is opened
/// `O_CREAT|O_EXCL|O_NOFOLLOW` and the rename replaces a link rather than following it, so a
/// symlink an agent planted at either name never redirects the write. `path`'s directory must
/// already be real ([`real_dir`]). A regular file that already holds `contents` with `mode` is
/// left as it is: a commit refreshes the hooks every time, and almost always finds them current.
fn write_replacing(path: &Path, contents: &str, mode: u32) -> Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    if fs::symlink_metadata(path).is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o7777 == mode)
        && read_regular(path).is_ok_and(|current| current == contents)
    {
        return Ok(());
    }
    let name = path.file_name().ok_or_else(|| anyhow!("{} has no file name", path.display()))?;
    let tmp = path.with_file_name(format!("{}.relay-tmp", name.to_string_lossy()));
    match fs::remove_file(&tmp) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).with_context(|| format!("removing {}", tmp.display())),
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&tmp)
        .with_context(|| format!("creating {}", tmp.display()))?;
    file.write_all(contents.as_bytes()).with_context(|| format!("writing {}", tmp.display()))?;
    drop(file);
    // `mode` passed to open is masked by the umask; an executable hook must be executable.
    fs::set_permissions(&tmp, fs::Permissions::from_mode(mode))?;
    fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))
}

fn write_executable(path: &Path, contents: &str) -> Result<()> {
    write_replacing(path, contents, 0o755)
}

/// Forward every hook but [`OWN_HOOKS`] from `hook_dir` to the user's `chain` directory: the
/// fixed [`FORWARDED_HOOKS`], plus any other executable hook `chain` holds right now. Stale
/// forwarders from an earlier install are removed.
fn write_forwarders(hook_dir: &Path, chain: Option<&Path>) -> Result<()> {
    let mut names: Vec<String> = Vec::new();
    if let Some(chain) = chain {
        names.extend(FORWARDED_HOOKS.iter().map(|name| name.to_string()));
        for entry in fs::read_dir(chain).into_iter().flatten().flatten() {
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else { continue };
            if !OWN_HOOKS.contains(&name.as_str()) && !name.contains('.') && !names.contains(&name) && is_executable(&entry.path()) {
                names.push(name);
            }
        }
        for name in &names {
            let script = format!(
                "#!/bin/sh\nhook={}\nif [ -x \"$hook\" ]; then exec \"$hook\" \"$@\"; fi\nexit 0\n",
                shell_quote_path(&chain.join(name)),
            );
            write_executable(&hook_dir.join(name), &script)?;
        }
    }
    for entry in fs::read_dir(hook_dir).into_iter().flatten().flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else { continue };
        if !OWN_HOOKS.contains(&name.as_str()) && !name.starts_with('.') && !names.contains(&name) && entry.path().is_file() {
            let _ = fs::remove_file(entry.path());
        }
    }
    Ok(())
}

/// Give one worktree its own hook path, preserving any pre-existing pre-commit hook by
/// chaining it before Relay's gate, and forwarding every other hook to where it lived.
pub fn install_git(
    repo: &Path,
    worktree: &Path,
    session: &str,
    instance: Instance,
    relay: &Path,
) -> Result<()> {
    // Read first: setting it rewrites the shared `.git/config` even when it is already on, and
    // fails outright while any other git command holds the config lock.
    if git_optional(repo, &["config", "--type=bool", "--get", "extensions.worktreeConfig"]).as_deref() != Some("true") {
        git(repo, &["config", "extensions.worktreeConfig", "true"])?;
    }
    // `.relay/` and the provider adapters land in the primary checkout too, which no
    // `worktree::create` ever excluded them from; a commit of everything would sweep them in.
    crate::worktree::ensure_excluded(repo).context("excluding Relay's files from git")?;

    let before = previous_hooks(repo, worktree);
    // In the primary checkout `.relay/` is as writable to the agent as any other file.
    let hook_dir = real_dir(repo, &format!(".relay/hooks/{session}"))?;
    let hook_path = hook_dir.join("pre-commit");
    let previous_hook = before.chain.as_ref().map(|dir| dir.join("pre-commit"));

    let user_payload = serde_json::to_string(&json!({"session": session, "kind": "commit"}))?;
    let previous = previous_hook
        .as_deref()
        .map(shell_quote_path)
        .unwrap_or_else(|| "''".to_string());
    let script = format!(
        "#!/bin/sh\n\
         set -u\n\
         previous={previous}\n\
         if [ -n \"$previous\" ] && [ -x \"$previous\" ]; then\n\
           \"$previous\" \"$@\" || exit $?\n\
         fi\n\
         relay_bin={}\n\
         if [ -n \"${{RELAY_SESSION:-}}\" ]; then\n\
           if [ -z \"${{RELAY_TOKEN:-}}\" ]; then\n\
             echo 'RELAY guardrail: session identity has no token' >&2\n\
             exit 2\n\
           fi\n\
           payload=$(printf '{{\"session\":\"%s\",\"kind\":\"commit\"}}' \"$RELAY_SESSION\")\n\
           exec \"$relay_bin\" --instance {} cmd guardrail.gate \"$payload\"\n\
         fi\n\
         exec \"$relay_bin\" --instance {} --actor user cmd guardrail.gate {}\n",
        shell_quote_path(relay),
        shell_quote(instance.as_str()),
        shell_quote(instance.as_str()),
        shell_quote(&user_payload),
    );
    write_executable(&hook_path, &script)?;
    write_executable(&hook_dir.join("pre-push"), &pre_push_script(before.chain.as_deref(), instance, relay))?;
    // Always written, empty when the checkout had no hook path of its own: uninstalling then
    // unsets it instead of pinning whatever was in effect.
    write_replacing(
        &hook_dir.join(PREVIOUS_HOOKS_PATH),
        &format!("{}\n", before.config.as_deref().unwrap_or_default()),
        0o644,
    )?;
    write_forwarders(&hook_dir, before.chain.as_deref())?;
    write_replacing(&hook_dir.join(HOOK_OWNER), &format!("{}\n", instance.as_str()), 0o644)?;

    let hook_dir_s = hook_dir.display().to_string();
    if before.current.as_deref() != Some(hook_dir_s.as_str()) {
        git(
            worktree,
            &["config", "--worktree", "core.hooksPath", &hook_dir_s],
        )?;
    }
    Ok(())
}

/// The pre-push hook: the user's own pre-push first, with the same arguments and refs on
/// stdin, then — for an agent's push only — `relay hook pre-push`, which runs the project's
/// check and refuses the push when it fails. A person's push (the Git panel's, from the
/// engine, has no `RELAY_SESSION`) runs only their own hook. A `relay` older than the check
/// (`RELAY_BIN` naming another checkout's build) lets the push through rather than fail it on
/// an unknown subcommand.
fn pre_push_script(chain: Option<&Path>, instance: Instance, relay: &Path) -> String {
    let previous = chain
        .map(|dir| shell_quote_path(&dir.join("pre-push")))
        .unwrap_or_else(|| "''".to_string());
    let relay = shell_quote_path(relay);
    format!(
        "#!/bin/sh\n\
         set -u\n\
         previous={previous}\n\
         refs=$(mktemp) || exit 1\n\
         trap 'rm -f \"$refs\"' EXIT\n\
         cat > \"$refs\"\n\
         if [ -n \"$previous\" ] && [ -x \"$previous\" ]; then\n\
           \"$previous\" \"$@\" < \"$refs\" || exit $?\n\
         fi\n\
         [ -n \"${{RELAY_SESSION:-}}\" ] || exit 0\n\
         {relay} hook pre-push --help >/dev/null 2>&1 || exit 0\n\
         {relay} --instance {} hook pre-push < \"$refs\"\n",
        shell_quote(instance.as_str()),
    )
}

/// Rewrite the currently active Relay-owned hook with the current CLI path. Existing worktrees can
/// outlive the Relay process that created them, so a commit is also a repair point for stale hooks.
/// Takes [`writes`]; only what is out of date is rewritten.
pub fn refresh_git(repo: &Path, worktree: &Path, instance: Instance, relay: &Path) -> Result<bool> {
    let Some(configured) = git_optional(
        worktree,
        &["config", "--worktree", "--get", "core.hooksPath"],
    ) else {
        return Ok(false);
    };
    let configured = PathBuf::from(configured);
    let absolute = if configured.is_absolute() { configured } else { worktree.join(configured) };
    let relay_hooks = repo.join(".relay").join("hooks");
    let Ok(relative) = absolute.strip_prefix(&relay_hooks) else {
        return Ok(false);
    };
    let mut components = relative.components();
    let Some(std::path::Component::Normal(session)) = components.next() else {
        return Ok(false);
    };
    if components.next().is_some() {
        return Ok(false);
    }
    let session = session
        .to_str()
        .ok_or_else(|| anyhow!("Relay hook session name is not UTF-8"))?;
    let _writes = writes();
    install_git(repo, worktree, session, instance, relay)?;
    Ok(true)
}

/// Run the user-owned pre-commit hook without re-entering Relay's generated guardrail hook.
/// `git.commit` already executes the guardrail in-process while its transaction is open; letting
/// the generated hook call the socket door here would wait on that same transaction forever.
/// A person's own pre-commit hook (lint, tests) may be slow, but not unbounded.
const USER_HOOK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);

pub fn run_user_pre_commit(repo: &Path, worktree: &Path) -> Result<()> {
    let Some(dir) = previous_hooks(repo, worktree).chain else { return Ok(()) };
    run_user_hook(&dir.join("pre-commit"), worktree, &[])
}

/// Run the user's own commit-msg hook on `message` and return the message as the hook left it
/// (a hook may rewrite it, as Gerrit's adds a Change-Id). `git.commit` commits with
/// `--no-verify` or `commit-tree`, neither of which runs commit-msg, so it calls this right
/// after [`run_user_pre_commit`] and commits what comes back. With no such hook the message is
/// returned unchanged.
pub fn run_user_commit_msg(repo: &Path, worktree: &Path, message: &str) -> Result<String> {
    run_user_message_hook(repo, worktree, "commit-msg", message, &[])
}

/// Run the user's own prepare-commit-msg hook on `message`, with the arguments `git commit -m`
/// gives it (the message file, then `message` as its source, also when concluding a merge), and
/// return the message as the hook left it. Only for a commit made with `commit-tree`: `git
/// commit --no-verify` runs prepare-commit-msg itself, since `--no-verify` skips only pre-commit
/// and commit-msg. Runs before [`run_user_commit_msg`], as in git.
pub fn run_user_prepare_commit_msg(repo: &Path, worktree: &Path, message: &str) -> Result<String> {
    run_user_message_hook(repo, worktree, "prepare-commit-msg", message, &["message".as_ref()])
}

fn run_user_message_hook(repo: &Path, worktree: &Path, name: &str, message: &str, args: &[&std::ffi::OsStr]) -> Result<String> {
    let Some(dir) = previous_hooks(repo, worktree).chain else { return Ok(message.to_string()) };
    let hook = dir.join(name);
    if !is_executable(&hook) {
        return Ok(message.to_string());
    }
    // The file `git commit` itself hands the hook: per checkout, so concurrent commits in two
    // worktrees never share it.
    let file = absolute_in(worktree, git(worktree, &["rev-parse", "--git-path", "COMMIT_EDITMSG"])?);
    let mut text = message.to_string();
    if !text.ends_with('\n') {
        text.push('\n');
    }
    write_replacing(&file, &text, 0o644)?;
    let mut all = vec![file.as_os_str()];
    all.extend_from_slice(args);
    run_user_hook(&hook, worktree, &all)?;
    read_regular(&file).with_context(|| format!("reading {}", file.display()))
}

fn run_user_hook(hook: &Path, worktree: &Path, args: &[&std::ffi::OsStr]) -> Result<()> {
    if !is_executable(hook) { return Ok(()) }
    let mut command = Command::new(hook);
    // A hook that fetches or asks GitHub fails instead of waiting on a prompt nobody sees: git
    // asks on /dev/tty even with stdin closed (RA-372).
    command.args(args).current_dir(worktree).env("GIT_TERMINAL_PROMPT", "0").env("GH_PROMPT_DISABLED", "1");
    let output = crate::proc::output_with_timeout(&mut command, USER_HOOK_TIMEOUT)
        .with_context(|| format!("running {}", hook.display()))?
        .ok_or_else(|| anyhow!("{} did not finish within {} s", hook.display(), USER_HOOK_TIMEOUT.as_secs()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if stderr.trim().is_empty() { stdout.trim() } else { stderr.trim() };
        return Err(anyhow!("{} failed with {}: {}", hook.display(), output.status, detail));
    }
    Ok(())
}

/// Restore the hook path that was active before Relay owned this worktree. If another Relay
/// session has since taken ownership, leave it alone.
pub fn uninstall_git(repo: &Path, worktree: &Path, session: &str) -> Result<()> {
    uninstall_git_any(repo, worktree, &[session.to_string()])
}

/// [`uninstall_git`] for every session that ever owned `worktree`, reading the active hook
/// path once instead of once per session. Only the session whose hook directory is active has
/// anything to undo: [`install_git`] records the pre-Relay path even when it replaces another
/// Relay hook, so restoring that one leaves every other name a no-op.
pub fn uninstall_git_any(repo: &Path, worktree: &Path, sessions: &[String]) -> Result<()> {
    if !worktree.exists() {
        return Ok(());
    }
    let relay_hooks = repo.join(".relay").join("hooks");
    // Bounded by the list: a previous path that is itself a listed Relay hook unwinds too.
    for _ in 0..sessions.len() {
        let Some(current) = git_optional(
            worktree,
            &["config", "--worktree", "--get", "core.hooksPath"],
        ) else {
            return Ok(());
        };
        let current = PathBuf::from(current);
        let current_absolute = if current.is_absolute() { current } else { worktree.join(current) };
        let owned = current_absolute
            .strip_prefix(&relay_hooks)
            .ok()
            .and_then(|rest| rest.to_str())
            .is_some_and(|name| sessions.iter().any(|session| session == name));
        if !owned {
            return Ok(());
        }
        // A record that names the inherited directory came from an older Relay that recorded
        // the effective path; restoring it would pin it, so it unsets like an empty one.
        // Asked only when there is a record, so a close of a checkout that had none spawns nothing more.
        match read_record(&current_absolute)
            .filter(|previous| Some(absolute_in(worktree, previous)) != inherited_hooks_dir(worktree))
        {
            Some(previous) => {
                git(
                    worktree,
                    &["config", "--worktree", "core.hooksPath", &previous],
                )?;
            }
            None => {
                let output = run_git(worktree, &["config", "--worktree", "--unset", "core.hooksPath"])?;
                if !output.status.success() && output.status.code() != Some(5) {
                    return Err(anyhow!(
                        "git config --worktree --unset core.hooksPath failed: {}",
                        String::from_utf8_lossy(&output.stderr).trim()
                    ));
                }
                return Ok(());
            }
        }
    }
    Ok(())
}

/// Retire `closing`'s hook directory on a checkout other sessions still use. When the
/// checkout's `core.hooksPath` points at that directory, it is first re-pointed at a fresh one
/// for `survivor` — [`install_git`] reads the closing directory's record of the pre-Relay hook
/// path, so the user's own hook stays in the chain — and only then deleted. Deleting it in
/// place silently turns the commit gate off for everyone left on the checkout.
///
/// Call with [`writes`] held. If the hand-over fails the directory is kept, still working.
pub fn hand_over(repo: &Path, worktree: &Path, closing: &str, survivor: &str, instance: Instance, relay: &Path) -> Result<()> {
    let closing_dir = repo.join(".relay").join("hooks").join(closing);
    if worktree.exists() {
        let active = git_optional(worktree, &["config", "--worktree", "--get", "core.hooksPath"])
            .map(PathBuf::from)
            .map(|path| if path.is_absolute() { path } else { worktree.join(path) });
        if active.as_deref() == Some(closing_dir.as_path()) {
            install_git(repo, worktree, survivor, instance, relay)?;
        }
    }
    remove_hook_dir(repo, closing);
    Ok(())
}

/// Drop one session's generated hook directory. Best-effort, and only for a directory no
/// checkout points at: [`uninstall_git`] or [`hand_over`] re-points the checkout first.
pub fn remove_hook_dir(repo: &Path, session: &str) {
    let _ = fs::remove_dir_all(repo.join(".relay").join("hooks").join(session));
}

/// Remove generated hook directories with no session behind them. Called from the launch
/// fsck, where the live set is already known — never on a timer (SPEC §15). Only `instance`'s
/// own directories are judged: `live` lists its sessions alone, so another instance's would all
/// look dead. A directory with no owner (written before the marker existed) is kept too, since
/// deleting a live one silently turns its checkout's commit gate off; its next install marks it.
pub fn sweep_hook_dirs(repo: &Path, live: &[String], instance: Instance) -> usize {
    let dir = repo.join(".relay").join("hooks");
    let Ok(entries) = fs::read_dir(&dir) else { return 0 };
    let mut removed = 0;
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else { continue };
        if live.iter().any(|session| session == &name) {
            continue;
        }
        let owner = read_regular(&entry.path().join(HOOK_OWNER)).unwrap_or_default();
        if owner.trim() != instance.as_str() {
            continue;
        }
        if fs::remove_dir_all(entry.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// Merge Relay's enforcement and lifecycle adapters into Claude's local, untracked settings.
pub fn install_claude(worktree: &Path, instance: Instance, relay: &Path) -> Result<()> {
    let dir = real_dir(worktree, ".claude")?;
    let path = dir.join("settings.local.json");
    let mut root: Value = match read_regular(&path) {
        Ok(raw) => serde_json::from_str(&raw)
            .with_context(|| format!("parsing {}", path.display()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };
    let object = root
        .as_object_mut()
        .ok_or_else(|| anyhow!("{} must contain a JSON object", path.display()))?;
    let hooks = object.entry("hooks").or_insert_with(|| json!({}));
    let hooks = hooks
        .as_object_mut()
        .ok_or_else(|| anyhow!("{}.hooks must be an object", path.display()))?;
    // Relay's handlers are replaced, never kept: one left from an older launch may name a relay
    // binary or instance that is gone (the guardrail then fails open with exit 127). They are
    // known by the shape Relay generates ([`is_relay_handler`]), so a user's own command that
    // merely contains the words survives; the real guardrail is appended whatever else is there.
    strip_handlers(hooks, CLAUDE_HOOK_MARKER);
    let pre = hooks.entry("PreToolUse").or_insert_with(|| json!([]));
    let pre = pre
        .as_array_mut()
        .ok_or_else(|| anyhow!("{}.hooks.PreToolUse must be an array", path.display()))?;
    let command = format!(
        "{} --instance {} {}",
        shell_quote_path(relay),
        shell_quote(instance.as_str()),
        CLAUDE_PRE_TOOL,
    );
    pre.push(json!({
        "matcher": "Edit|Write|MultiEdit|Bash",
        "hooks": [{"type": "command", "command": command, "timeout": 30}]
    }));

    // A tool that failed reports as one that finished (a non-zero `adb install` included, or its
    // device lease outlived it), and a new prompt ends a turn that Esc interrupted, which sends
    // neither PostToolUse nor Stop.
    for (event, kind) in [
        ("SessionStart", "session_start"),
        ("PostToolUse", "tool_use"),
        ("PostToolUseFailure", "tool_use"),
        ("UserPromptSubmit", "prompt"),
        ("Stop", "stop"),
        ("Notification", "notification"),
    ] {
        let groups = hooks.entry(event).or_insert_with(|| json!([]));
        let groups = groups.as_array_mut()
            .ok_or_else(|| anyhow!("{}.hooks.{event} must be an array", path.display()))?;
        let command = format!(
            "{} --instance {} hook claude-report {}",
            shell_quote_path(relay), shell_quote(instance.as_str()), shell_quote(kind),
        );
        groups.push(json!({
            "hooks": [{"type": "command", "command": command, "timeout": 10}]
        }));
    }

    write_replacing(&path, &format!("{}\n", serde_json::to_string_pretty(&root)?), 0o644)?;

    let _ = fs::remove_file(worktree.join(LEGACY_CLAUDE_STATUSLINE_RELATIVE));
    let _ = fs::remove_file(worktree.join(LEGACY_CLAUDE_SETTINGS_RELATIVE));

    // Explicit per-launch MCP config: it inherits the session/token/project environment from
    // the Claude process, so no credentials are ever written to disk.
    let mcp_path = real_dir(worktree, ".relay")?.join("relay.mcp.json");
    let config = json!({"mcpServers":{"relay":{
        "command":relay.display().to_string(),
        "args":["--instance",instance.as_str(),"mcp"]
    }}});
    write_replacing(&mcp_path, &format!("{}\n", serde_json::to_string_pretty(&config)?), 0o644)?;
    Ok(())
}

/// Add the MCP servers of the project's enabled plugins to the per-launch config written by
/// [`install_claude`] (D159). Relay's own `relay` entry is never replaced.
pub fn add_claude_mcp_servers(
    worktree: &Path,
    servers: &[crate::plugins::LaunchServer],
) -> Result<()> {
    if servers.is_empty() {
        return Ok(());
    }
    let mcp_path = worktree.join(MCP_CONFIG_RELATIVE);
    let raw = read_regular(&mcp_path).with_context(|| format!("reading {}", mcp_path.display()))?;
    let mut config: Value = serde_json::from_str(&raw).with_context(|| format!("parsing {}", mcp_path.display()))?;
    let Some(entries) = config.get_mut("mcpServers").and_then(Value::as_object_mut) else {
        anyhow::bail!("{} has no mcpServers", mcp_path.display());
    };
    for (name, command, args, env) in servers {
        if name == "relay" {
            continue;
        }
        let mut entry = json!({"command": command, "args": args});
        if !env.is_empty() {
            entry["env"] = json!(env);
        }
        entries.insert(name.clone(), entry);
    }
    let mcp_path = real_dir(worktree, ".relay")?.join("relay.mcp.json");
    write_replacing(&mcp_path, &format!("{}\n", serde_json::to_string_pretty(&config)?), 0o644)?;
    Ok(())
}

/// Remove only Relay's Claude handlers; all user/project-local settings survive.
pub fn uninstall_claude(worktree: &Path) -> Result<()> {
    let _ = fs::remove_file(worktree.join(MCP_CONFIG_RELATIVE));
    let _ = fs::remove_file(worktree.join(LEGACY_CLAUDE_SETTINGS_RELATIVE));
    let _ = fs::remove_file(worktree.join(LEGACY_CLAUDE_STATUSLINE_RELATIVE));
    let dir = worktree.join(".claude");
    let path = dir.join("settings.local.json");
    let raw = match read_regular(&path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error).with_context(|| format!("reading {}", path.display())),
    };
    let mut root: Value = serde_json::from_str(&raw)
        .with_context(|| format!("parsing {}", path.display()))?;
    let Some(hooks) = root.get_mut("hooks").and_then(Value::as_object_mut)
    else {
        return Ok(());
    };
    strip_handlers(hooks, CLAUDE_HOOK_MARKER);
    let path = real_dir(worktree, ".claude")?.join("settings.local.json");
    write_replacing(&path, &format!("{}\n", serde_json::to_string_pretty(&root)?), 0o644)?;
    Ok(())
}

/// `install_codex`'s refusal: the repository tracks `.codex/hooks.json`.
#[derive(Debug)]
pub struct CodexHooksTracked(pub PathBuf);

impl std::fmt::Display for CodexHooksTracked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{CODEX_HOOKS_RELATIVE} is tracked by git in {}; Relay will not write its machine-specific \
             guardrail hooks into a file a commit would share. Untrack it to run Codex here",
            self.0.display(),
        )
    }
}

impl std::error::Error for CodexHooksTracked {}

/// Merge Relay's adapters into Codex's project hook layer. Codex asks the user to trust the
/// exact generated hook once through `/hooks`; Relay never bypasses that provider boundary.
///
/// Refused when the repository tracks `.codex/hooks.json`: Relay's handlers name this machine's
/// relay binary and instance, and the next `add -A` commit would hand them to every teammate,
/// whose Codex would then run a binary that is not there on every tool use. Launching Codex
/// without them instead would run it outside the guardrails. An untracked file is kept out of
/// git once Relay's handlers are in it, and only then: the path is not excluded up front, so a
/// `.codex/hooks.json` someone makes in a repository Codex never ran in is theirs to commit.
pub fn install_codex(worktree: &Path, instance: Instance, relay: &Path) -> Result<()> {
    let tracking = tracked(worktree, CODEX_HOOKS_RELATIVE)?;
    if tracking == Some(true) {
        return Err(CodexHooksTracked(worktree.to_path_buf()).into());
    }
    let path = real_dir(worktree, ".codex")?.join("hooks.json");
    let mut root: Value = match read_regular(&path) {
        Ok(raw) => serde_json::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => json!({
            "description": "Workspace hooks, including Relay guardrails. Review once with /hooks."
        }),
        Err(error) => return Err(error).with_context(|| format!("reading {}", path.display())),
    };
    if tracking == Some(false) {
        crate::worktree::exclude_paths(worktree, &[CODEX_HOOKS_RELATIVE])
            .context("excluding Relay's Codex hooks from git")?;
    }
    let object = root.as_object_mut()
        .ok_or_else(|| anyhow!("{} must contain a JSON object", path.display()))?;
    let hooks = object.entry("hooks").or_insert_with(|| json!({})).as_object_mut()
        .ok_or_else(|| anyhow!("{}.hooks must be an object", path.display()))?;

    // Replaced on every launch, as for Claude: a stale path must not survive, nor a look-alike.
    strip_handlers(hooks, CODEX_HOOK_MARKER);
    let pre = hooks.entry("PreToolUse").or_insert_with(|| json!([])).as_array_mut()
        .ok_or_else(|| anyhow!("{}.hooks.PreToolUse must be an array", path.display()))?;
    pre.push(json!({
        "matcher": "Bash|apply_patch|Edit|Write",
        "hooks": [{
            "type": "command",
            "command": format!("{} --instance {} {}", shell_quote_path(relay), shell_quote(instance.as_str()), CODEX_PRE_TOOL),
            "timeout": 30,
            "statusMessage": "Checking Relay guardrails"
        }]
    }));

    for (event, kind) in [("SessionStart", "session_start"), ("PostToolUse", "tool_use"), ("Stop", "stop")] {
        let groups = hooks.entry(event).or_insert_with(|| json!([])).as_array_mut()
            .ok_or_else(|| anyhow!("{}.hooks.{event} must be an array", path.display()))?;
        groups.push(json!({
            "hooks": [{
                "type": "command",
                "command": format!("{} --instance {} hook codex-report {}", shell_quote_path(relay), shell_quote(instance.as_str()), shell_quote(kind)),
                "timeout": 10
            }]
        }));
    }

    write_replacing(&path, &format!("{}\n", serde_json::to_string_pretty(&root)?), 0o644)?;
    Ok(())
}

/// Remove only Relay's Codex handlers. User and project hook definitions remain byte-for-byte
/// equivalent as JSON values.
pub fn uninstall_codex(worktree: &Path) -> Result<()> {
    let path = worktree.join(CODEX_HOOKS_RELATIVE);
    let raw = match read_regular(&path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error).with_context(|| format!("reading {}", path.display())),
    };
    let mut root: Value = serde_json::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;
    let Some(hooks) = root.get_mut("hooks").and_then(Value::as_object_mut) else { return Ok(()) };
    strip_handlers(hooks, CODEX_HOOK_MARKER);
    let path = real_dir(worktree, ".codex")?.join("hooks.json");
    write_replacing(&path, &format!("{}\n", serde_json::to_string_pretty(&root)?), 0o644)?;
    Ok(())
}

/// Remove every handler Relay generated for `marker`'s provider, and each group that held only
/// those. A user's group that was already empty is left as it was.
fn strip_handlers(hooks: &mut serde_json::Map<String, Value>, marker: &str) {
    for groups in hooks.values_mut().filter_map(Value::as_array_mut) {
        groups.retain_mut(|group| {
            let Some(handlers) = group.get_mut("hooks").and_then(Value::as_array_mut) else { return true };
            let before = handlers.len();
            handlers.retain(|handler| !handler["command"].as_str().is_some_and(|command| is_relay_handler(command, marker)));
            before == 0 || !handlers.is_empty()
        });
    }
}

/// Whether `command` has the shape of a handler Relay generates, `<relay> --instance <name>
/// hook <provider>-…`, whatever binary or instance it names. `~/bin/notify-hook claude-done`
/// or `echo hook claude-done` is someone else's.
fn is_relay_handler(command: &str, marker: &str) -> bool {
    command.split_once(&format!(" {marker}")).is_some_and(|(head, _)| {
        let mut words = head.rsplitn(3, ' ');
        let (_instance, flag, relay) = (words.next(), words.next(), words.next());
        flag == Some("--instance") && relay.is_some_and(|relay| !relay.is_empty())
    })
}

/// Claude already receives account rate limits on each normal turn. A silent status-line
/// command captures that payload for `usage.get`; it makes no provider or network request. It is
/// a shell command line rather than a script file, so nothing in the checkout is executed.
fn claude_statusline_script() -> &'static str {
    "input=$(cat); case \"$input\" in *'\"rate_limits\"'*) ;; *) exit 0 ;; esac; \
     out=\"${CLAUDE_CONFIG_DIR:-$HOME}/.claude/relay-usage.json\"; mkdir -p \"${out%/*}\" 2>/dev/null; \
     tmp=\"$out.$$\"; { printf '%s\\n' \"$input\" > \"$tmp\" && mv -f \"$tmp\" \"$out\"; } 2>/dev/null || rm -f \"$tmp\"; exit 0"
}

/// Where `relay-key-proxy` listens (docs/KEY-PROXY.md). It holds the Anthropic API key as its own
/// system user, out of reach of everything that runs as the person; a `use_api_key` session's
/// Claude sends its requests there.
pub const KEY_PROXY_PORT: u16 = 7445;

/// The local pass token a session's Claude shows the key proxy, written there by
/// `scripts/key-proxy/install.sh`. It is not the key: it lets a program running as the person
/// spend through the proxy on this machine, and shows no one the key.
pub fn key_proxy_token_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config/relay/key-proxy-token"))
}

/// How a `use_api_key` session's Claude gets the pass token it puts in `x-api-key`.
pub fn api_key_helper(token: &Path) -> String {
    format!("/usr/bin/cat {}", shell_quote_path(token))
}

/// The `--settings` value for a Claude launch: inline JSON, which Claude accepts in place of a
/// file path, so the command it runs every turn cannot be rewritten by the agent it serves.
/// `api_key_helper` is set for a `use_api_key` session, which then talks to the key proxy; the
/// launch's own settings outrank the checkout's, so a repository cannot point it elsewhere, nor
/// through a proxy of its own (NO_PROXY keeps loopback direct). `reads` may be read without
/// asking: the store's design-assets folder (TM-5: the picture an Ask from the canvas hands the
/// session is there) and, with the Video plugin on, the videos folder (the kit's sources and
/// Remotion's types, D167). Only read: never written or edited, and not added as directories,
/// which would let it work there as in its checkout.
pub fn claude_settings(api_key_helper: Option<&str>, reads: &[&Path]) -> String {
    let mut settings = json!({"statusLine": {"type": "command", "command": claude_statusline_script(), "padding": 0}});
    // Claude's absolute-path rule: `//` then the path without its leading `/`.
    let allow: Vec<String> = reads.iter().filter(|dir| dir.is_absolute())
        .map(|dir| format!("Read(//{}/**)", dir.display().to_string().trim_start_matches('/')))
        .collect();
    if !allow.is_empty() {
        settings["permissions"] = json!({"allow": allow});
    }
    if let Some(helper) = api_key_helper {
        settings["apiKeyHelper"] = json!(helper);
        settings["env"] = json!({
            "ANTHROPIC_BASE_URL": format!("http://127.0.0.1:{KEY_PROXY_PORT}"),
            "NO_PROXY": "127.0.0.1,localhost",
            "no_proxy": "127.0.0.1,localhost",
        });
    }
    settings.to_string()
}

/// Hook setup is a handful of local `git config` calls; one that outlives this is stuck.
const GIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// How often a `git config` write that found `config.lock` taken is tried again. Agents run
/// git in the same repository (`git push -u` writes the shared config), so a launch or commit
/// can meet a lock that is held for a moment; it is not worth failing over.
const CONFIG_LOCK_RETRIES: u32 = 3;

/// Run git in `repo` within [`GIT_TIMEOUT`], retrying a moment later while another git process
/// holds the config lock.
fn run_git(repo: &Path, args: &[&str]) -> Result<std::process::Output> {
    let mut attempt = 0;
    loop {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(repo).args(args);
        let output = crate::proc::output_with_timeout(&mut cmd, GIT_TIMEOUT)
            .with_context(|| format!("running git {}", args.join(" ")))?
            .ok_or_else(|| anyhow!("git {} timed out", args.join(" ")))?;
        if output.status.success() || attempt == CONFIG_LOCK_RETRIES
            || !String::from_utf8_lossy(&output.stderr).contains("could not lock config file")
        {
            return Ok(output);
        }
        attempt += 1;
        std::thread::sleep(std::time::Duration::from_millis(50 * u64::from(attempt)));
    }
}

fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let output = run_git(repo, args)?;
    if !output.status.success() {
        return Err(anyhow!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Whether git tracks `relative` in the checkout at `worktree`, a file only staged included;
/// `None` outside a repository.
fn tracked(worktree: &Path, relative: &str) -> Result<Option<bool>> {
    let output = run_git(worktree, &["ls-files", "--error-unmatch", "--", relative])?;
    // `--error-unmatch` exits 1 for a path git does not know; 128 is no repository at all.
    Ok(match output.status.code() {
        Some(0) => Some(true),
        Some(1) => Some(false),
        _ => None,
    })
}

fn git_optional(repo: &Path, args: &[&str]) -> Option<String> {
    git(repo, args).ok().filter(|value| !value.is_empty())
}

fn shell_quote_path(path: &Path) -> String {
    shell_quote(&path.display().to_string())
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::{claude_statusline_script, install_claude, install_git, refresh_git, relay_bin_from,
        remove_hook_dir, run_user_commit_msg, shell_quote, sweep_hook_dirs, uninstall_git};
    use crate::Instance;
    use serde_json::json;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::process::Command;

    #[test]
    fn shell_quotes_single_quotes() {
        assert_eq!(shell_quote("a'b"), "'a'\"'\"'b'");
    }

    #[test]
    fn statusline_is_silent_and_captures_rate_limits() {
        let script = claude_statusline_script();
        assert!(script.contains("rate_limits"));
        assert!(script.contains("relay-usage.json"));
        assert!(!script.contains("curl"));
    }

    #[test]
    fn relay_bin_finds_cli_beside_desktop_executable() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("relay-app");
        let cli = dir.path().join("relay");
        std::fs::write(&cli, "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();

        assert_eq!(relay_bin_from(None, Some(app)), cli);
    }

    #[test]
    fn stale_hook_directories_are_swept_and_live_ones_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        for session in ["live-otter", "dead-koala", "dead-ferret", "stable-heron", "legacy-mole"] {
            std::fs::create_dir_all(repo.join(".relay/hooks").join(session)).unwrap();
            let owner = if session == "stable-heron" { "stable" } else { "test" };
            if session != "legacy-mole" {
                std::fs::write(repo.join(".relay/hooks").join(session).join(super::HOOK_OWNER), format!("{owner}\n")).unwrap();
            }
        }
        std::fs::write(repo.join(".relay/hooks/not-a-dir"), "").unwrap();

        assert_eq!(sweep_hook_dirs(repo, &["live-otter".to_string()], Instance::Test), 2);
        assert!(repo.join(".relay/hooks/live-otter").is_dir());
        assert!(!repo.join(".relay/hooks/dead-koala").exists());
        assert!(repo.join(".relay/hooks/stable-heron").is_dir(), "another instance's live hook was swept");
        assert!(repo.join(".relay/hooks/legacy-mole").is_dir(), "an unowned hook may be live");
        assert!(repo.join(".relay/hooks/not-a-dir").is_file(), "only directories are swept");

        remove_hook_dir(repo, "live-otter");
        assert!(!repo.join(".relay/hooks/live-otter").exists());
        // A repository with no hook directory at all is not an error.
        assert_eq!(sweep_hook_dirs(&repo.join("nowhere"), &[], Instance::Test), 0);
    }

    #[test]
    fn refresh_git_rewrites_an_existing_relay_hook() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        let init = Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["init", "-b", "main"])
            .output()
            .unwrap();
        assert!(init.status.success());

        install_git(&repo, &repo, "steady-fox", Instance::Test, Path::new("/bin/false")).unwrap();
        assert!(refresh_git(&repo, &repo, Instance::Test, Path::new("/bin/true")).unwrap());

        let hook = repo.join(".relay/hooks/steady-fox/pre-commit");
        let script = std::fs::read_to_string(&hook).unwrap();
        assert!(script.contains("relay_bin='/bin/true'"));
        assert!(Command::new(&hook).status().unwrap().success());
    }

    fn init_repo(dir: &Path) -> std::path::PathBuf {
        let repo = dir.join("repo");
        std::fs::create_dir(&repo).unwrap();
        let init = Command::new("git").arg("-C").arg(&repo).args(["init", "-q", "-b", "main"]).output().unwrap();
        assert!(init.status.success());
        repo
    }

    fn hooks_path(repo: &Path) -> Option<String> {
        let out = Command::new("git").arg("-C").arg(repo)
            .args(["config", "--worktree", "--get", "core.hooksPath"]).output().unwrap();
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    fn executable(path: &Path, body: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn statusline_command_records_rate_limits_and_ignores_the_rest() {
        let home = tempfile::tempdir().unwrap();
        let run = |input: &str| {
            use std::io::Write;
            let mut child = Command::new("sh").arg("-c").arg(claude_statusline_script())
                .env("HOME", home.path()).env_remove("CLAUDE_CONFIG_DIR")
                .stdin(std::process::Stdio::piped()).spawn().unwrap();
            child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
            assert!(child.wait().unwrap().success());
        };
        let out = home.path().join(".claude/relay-usage.json");
        run("{\"model\":{}}");
        assert!(!out.exists());
        run("{\"rate_limits\":{\"five_hour\":1}}");
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "{\"rate_limits\":{\"five_hour\":1}}\n");
        // Written beside and renamed into place, so a reader never sees half of it.
        assert_eq!(std::fs::read_dir(out.parent().unwrap()).unwrap().count(), 1, "a temp file was left behind");
        let settings: serde_json::Value = serde_json::from_str(&super::claude_settings(None, &[])).unwrap();
        assert_eq!(settings["statusLine"]["command"], claude_statusline_script());
        assert!(settings.get("apiKeyHelper").is_none() && settings.get("env").is_none(), "a subscription session must not be pointed at the key");
        assert!(settings.get("permissions").is_none());
        // TM-5: the design pictures may be read without asking, and only read.
        let settings: serde_json::Value = serde_json::from_str(&super::claude_settings(None, &[Path::new("/home/a b/.local/share/relay-v4/dev/design-assets")])).unwrap();
        assert_eq!(settings["permissions"], json!({"allow": ["Read(//home/a b/.local/share/relay-v4/dev/design-assets/**)"]}));
        let text = settings.to_string();
        assert!(!text.contains("Write(") && !text.contains("Edit(") && !text.contains("additionalDirectories"), "{text}");
        // D167: with Video on, the videos folder (the kit, Remotion's types) as well; a relative path never.
        let settings: serde_json::Value = serde_json::from_str(&super::claude_settings(None,
            &[Path::new("/data/design-assets"), Path::new("/data/videos"), Path::new("relative")])).unwrap();
        assert_eq!(settings["permissions"], json!({"allow": ["Read(//data/design-assets/**)", "Read(//data/videos/**)"]}));
        let helper = super::api_key_helper(Path::new("/home/a b/.config/relay/key-proxy-token"));
        assert_eq!(helper, "/usr/bin/cat '/home/a b/.config/relay/key-proxy-token'");
        let settings: serde_json::Value = serde_json::from_str(&super::claude_settings(Some(&helper), &[])).unwrap();
        assert_eq!(settings["apiKeyHelper"], helper);
        assert_eq!(settings["env"], json!({"ANTHROPIC_BASE_URL": format!("http://127.0.0.1:{}", super::KEY_PROXY_PORT),
            "NO_PROXY": "127.0.0.1,localhost", "no_proxy": "127.0.0.1,localhost"}));
        assert_eq!(settings["statusLine"]["command"], claude_statusline_script());
        // The proxy's socket listens where sessions are sent (the proxy's own test checks its side).
        let socket = include_str!("../../../scripts/key-proxy/relay-key-proxy.socket");
        assert!(socket.contains(&format!("ListenStream=127.0.0.1:{}\n", super::KEY_PROXY_PORT)));
    }

    /// A checkout with no hook path of its own gets none back, not the one that was in effect;
    /// one with its own gets exactly that back; and a record an older Relay wrote with the
    /// effective directory no longer pins it.
    #[test]
    fn uninstall_restores_the_checkouts_own_hook_path_or_none() {
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo(dir.path());
        install_git(&repo, &repo, "calm-otter", Instance::Test, Path::new("/bin/true")).unwrap();
        assert!(hooks_path(&repo).unwrap().ends_with(".relay/hooks/calm-otter"));
        uninstall_git(&repo, &repo, "calm-otter").unwrap();
        assert_eq!(hooks_path(&repo), None, "uninstall pinned a hook path the checkout never had");

        let set = Command::new("git").arg("-C").arg(&repo)
            .args(["config", "--worktree", "core.hooksPath", "user-hooks"]).status().unwrap();
        assert!(set.success());
        install_git(&repo, &repo, "calm-otter", Instance::Test, Path::new("/bin/true")).unwrap();
        install_git(&repo, &repo, "sly-egret", Instance::Test, Path::new("/bin/true")).unwrap();
        uninstall_git(&repo, &repo, "sly-egret").unwrap();
        assert_eq!(hooks_path(&repo).as_deref(), Some("user-hooks"));

        let unset = Command::new("git").arg("-C").arg(&repo)
            .args(["config", "--worktree", "--unset", "core.hooksPath"]).status().unwrap();
        assert!(unset.success());
        install_git(&repo, &repo, "old-relay", Instance::Test, Path::new("/bin/true")).unwrap();
        std::fs::write(repo.join(".relay/hooks/old-relay").join(super::PREVIOUS_HOOKS_PATH), ".git/hooks\n").unwrap();
        uninstall_git(&repo, &repo, "old-relay").unwrap();
        assert_eq!(hooks_path(&repo), None, "a legacy record of the default directory was pinned");
    }

    #[test]
    fn every_other_hook_reaches_the_users_directory() {
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo(dir.path());
        let user = repo.join(".git/hooks");
        executable(&user.join("pre-push"), "#!/bin/sh\necho \"$@\" > \"$GIT_DIR_OUT\"\ncat >> \"$GIT_DIR_OUT\"\nexit 3\n");
        executable(&user.join("reference-transaction"), "#!/bin/sh\nexit 0\n");
        install_git(&repo, &repo, "calm-otter", Instance::Test, Path::new("/bin/true")).unwrap();
        let relay = repo.join(".relay/hooks/calm-otter");
        assert!(relay.join("reference-transaction").is_file(), "a hook the user has was not forwarded");
        assert!(!relay.join("pre-push.sample").exists());

        // Arguments, stdin and the exit status all pass through.
        use std::io::Write;
        let out = dir.path().join("pre-push.out");
        let mut child = Command::new(relay.join("pre-push")).args(["origin", "url"])
            .env("GIT_DIR_OUT", &out).stdin(std::process::Stdio::piped()).spawn().unwrap();
        child.stdin.take().unwrap().write_all(b"refs/heads/main sha\n").unwrap();
        assert_eq!(child.wait().unwrap().code(), Some(3));
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "origin url\nrefs/heads/main sha\n");
        // A hook the user does not have is a no-op, as it would be for git.
        assert!(Command::new(relay.join("post-checkout")).status().unwrap().success());

        std::fs::remove_file(user.join("reference-transaction")).unwrap();
        install_git(&repo, &repo, "calm-otter", Instance::Test, Path::new("/bin/true")).unwrap();
        assert!(!relay.join("reference-transaction").exists(), "a stale forwarder survived");
        assert!(relay.join(super::PREVIOUS_HOOKS_PATH).is_file());
    }

    #[test]
    fn an_agents_push_runs_the_check_after_the_users_own_hook() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo(dir.path());
        let out = dir.path().join("pre-push.out");
        let relay = dir.path().join("fake-relay");
        executable(&relay, "#!/bin/sh\ncase \"$*\" in *--help) exit 0;; esac\necho \"relay $*\" >> \"$GIT_DIR_OUT\"\ncat >> \"$GIT_DIR_OUT\"\nexit 1\n");
        let push = |session: Option<&str>| {
            let mut command = Command::new(repo.join(".relay/hooks/calm-otter/pre-push"));
            command.args(["origin", "url"]).env("GIT_DIR_OUT", &out).env_remove("RELAY_SESSION").stdin(std::process::Stdio::piped());
            if let Some(session) = session {
                command.env("RELAY_SESSION", session);
            }
            let mut child = command.spawn().unwrap();
            child.stdin.take().unwrap().write_all(b"refs/heads/main sha\n").unwrap();
            child.wait().unwrap().code()
        };

        install_git(&repo, &repo, "calm-otter", Instance::Test, &relay).unwrap();
        assert_eq!(push(None), Some(0), "a person's push ran the check");
        assert!(!out.exists());
        assert_eq!(push(Some("calm-otter")), Some(1), "a failed check let the push through");
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "relay --instance test hook pre-push\nrefs/heads/main sha\n");

        // The user's own pre-push runs first, sees the same refs, and a refusal there ends it.
        std::fs::remove_file(&out).unwrap();
        executable(&repo.join(".git/hooks/pre-push"), "#!/bin/sh\necho \"user $*\" >> \"$GIT_DIR_OUT\"\ncat >> \"$GIT_DIR_OUT\"\n");
        install_git(&repo, &repo, "calm-otter", Instance::Test, &relay).unwrap();
        assert_eq!(push(Some("calm-otter")), Some(1));
        assert_eq!(
            std::fs::read_to_string(&out).unwrap(),
            "user origin url\nrefs/heads/main sha\nrelay --instance test hook pre-push\nrefs/heads/main sha\n",
        );
        std::fs::remove_file(&out).unwrap();
        executable(&repo.join(".git/hooks/pre-push"), "#!/bin/sh\nexit 4\n");
        assert_eq!(push(Some("calm-otter")), Some(4));
        assert!(!out.exists(), "the check ran after the user's hook refused");

        // A relay too old to know the check lets the push through instead of failing it.
        std::fs::remove_file(repo.join(".git/hooks/pre-push")).unwrap();
        install_git(&repo, &repo, "calm-otter", Instance::Test, Path::new("/bin/false")).unwrap();
        assert_eq!(push(Some("calm-otter")), Some(0));
    }

    #[test]
    fn relay_files_are_excluded_in_the_primary_checkout() {
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo(dir.path());
        install_git(&repo, &repo, "calm-otter", Instance::Test, Path::new("/bin/true")).unwrap();
        install_claude(&repo, Instance::Test, Path::new("/bin/true")).unwrap();
        let status = Command::new("git").arg("-C").arg(&repo).args(["status", "--porcelain", "--untracked-files=all"]).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&status.stdout), "", "Relay's files would be committed");
    }

    #[test]
    fn commit_msg_hook_can_refuse_or_rewrite_a_bus_commit_message() {
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo(dir.path());
        install_git(&repo, &repo, "calm-otter", Instance::Test, Path::new("/bin/true")).unwrap();
        assert_eq!(run_user_commit_msg(&repo, &repo, "no hook").unwrap(), "no hook");

        executable(&repo.join(".git/hooks/commit-msg"),
            "#!/bin/sh\ngrep -q WIP \"$1\" && { echo 'no WIP commits' >&2; exit 1; }\nprintf '\\nChange-Id: I123\\n' >> \"$1\"\n");
        assert_eq!(run_user_commit_msg(&repo, &repo, "Fix the thing").unwrap(), "Fix the thing\n\nChange-Id: I123\n");
        let refused = run_user_commit_msg(&repo, &repo, "WIP").unwrap_err().to_string();
        assert!(refused.contains("no WIP commits"), "{refused}");
    }

    /// A handler from an older launch names a relay binary that may be gone, and an agent can
    /// write one that only mentions the marker; neither may stand in for the real guardrail.
    #[test]
    fn install_claude_replaces_stale_and_look_alike_relay_handlers() {
        let dir = tempfile::tempdir().unwrap();
        let worktree = dir.path();
        std::fs::create_dir_all(worktree.join(".claude")).unwrap();
        std::fs::write(worktree.join(".claude/settings.local.json"), serde_json::json!({"hooks": {
            "PreToolUse": [
                {"matcher": "Bash", "hooks": [{"type": "command", "command": "true --instance 'dev' hook claude-pre-tool"}]},
                {"matcher": "Read", "hooks": [{"type": "command", "command": "mine"}]},
                {"matcher": "Glob", "hooks": []},
            ],
            "Stop": [
                {"hooks": [{"type": "command", "command": "'/gone/relay' --instance 'dev' hook claude-report 'stop'"}]},
                {"hooks": [{"type": "command", "command": "~/bin/notify-hook claude-done"}, {"type": "command", "command": "echo hook claude-done"}]},
            ],
        }}).to_string()).unwrap();
        std::fs::create_dir_all(worktree.join(".relay")).unwrap();
        std::fs::write(worktree.join(".relay/agent-statusline.sh"), "#!/bin/sh\nrm -rf /\n").unwrap();

        install_claude(worktree, Instance::Test, Path::new("/opt/relay")).unwrap();
        let settings: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(worktree.join(".claude/settings.local.json")).unwrap()).unwrap();
        let pre = settings["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 3, "{pre:?}");
        assert_eq!(pre[0]["matcher"], "Read");
        assert_eq!(pre[1]["matcher"], "Glob", "a user's empty group is theirs to keep");
        assert_eq!(pre[2]["hooks"][0]["command"], "'/opt/relay' --instance 'test' hook claude-pre-tool");
        let stop = settings["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 2, "a user's own command that mentions the words was stripped: {stop:?}");
        assert_eq!(stop[0]["hooks"].as_array().unwrap().len(), 2);
        assert!(stop[1]["hooks"][0]["command"].as_str().unwrap().starts_with("'/opt/relay' --instance 'test'"));

        // Close takes Relay's handlers out and leaves the user's.
        super::uninstall_claude(worktree).unwrap();
        let settings: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(worktree.join(".claude/settings.local.json")).unwrap()).unwrap();
        assert_eq!(settings["hooks"]["Stop"].as_array().unwrap().len(), 1);
        assert_eq!(settings["hooks"]["Stop"][0]["hooks"][1]["command"], "echo hook claude-done");
        assert_eq!(settings["hooks"]["PreToolUse"].as_array().unwrap().len(), 2);
        assert!(!worktree.join(".relay/agent-statusline.sh").exists(), "the old executable status line survived");
    }

    /// A commit refreshes the hook on every call; when nothing changed it must not rewrite the
    /// shared `.git/config`, whose lock an agent's own `git push -u` may hold at that moment.
    #[test]
    fn refresh_git_leaves_a_current_hook_and_config_alone() {
        use std::os::unix::fs::MetadataExt;
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo(dir.path());
        install_git(&repo, &repo, "calm-otter", Instance::Test, Path::new("/bin/true")).unwrap();
        let config = repo.join(".git/config");
        let hook = repo.join(".relay/hooks/calm-otter/pre-commit");
        let before = (std::fs::metadata(&config).unwrap().ino(), std::fs::metadata(&hook).unwrap().ino());
        assert!(refresh_git(&repo, &repo, Instance::Test, Path::new("/bin/true")).unwrap());
        let after = (std::fs::metadata(&config).unwrap().ino(), std::fs::metadata(&hook).unwrap().ino());
        assert_eq!(before, after, "an unchanged hook or config was rewritten");

        // A held config lock is waited out, not failed over.
        // `core.hooksPath` goes to the worktree config, so that is the lock in the way.
        let lock = repo.join(".git/config.worktree.lock");
        std::fs::write(&lock, "").unwrap();
        let release = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(80));
            std::fs::remove_file(lock).unwrap();
        });
        install_git(&repo, &repo, "sly-egret", Instance::Test, Path::new("/bin/true")).unwrap();
        release.join().unwrap();
        assert!(hooks_path(&repo).unwrap().ends_with(".relay/hooks/sly-egret"));
    }

    #[test]
    fn relay_handlers_are_known_by_their_generated_shape() {
        use super::{is_relay_handler, CLAUDE_HOOK_MARKER, CODEX_HOOK_MARKER};
        assert!(is_relay_handler("'/opt/my relay' --instance 'dev' hook claude-pre-tool", CLAUDE_HOOK_MARKER));
        assert!(is_relay_handler("relay --instance stable hook claude-report 'stop'", CLAUDE_HOOK_MARKER));
        assert!(is_relay_handler("'/opt/relay' --instance 'dev' hook codex-pre-tool", CODEX_HOOK_MARKER));
        assert!(!is_relay_handler("'/opt/relay' --instance 'dev' hook codex-pre-tool", CLAUDE_HOOK_MARKER));
        for mine in ["~/bin/notify-hook claude-done", "echo hook claude-done", "true # hook claude-pre-tool", "hook claude-x"] {
            assert!(!is_relay_handler(mine, CLAUDE_HOOK_MARKER), "{mine}");
        }
    }

    /// A repository that commits its Codex hooks never gets Relay's machine-specific ones
    /// written into the file; an untracked one is excluded only once Relay's handlers are in it.
    #[test]
    fn codex_hooks_are_refused_in_a_tracked_file_and_excluded_only_once_written() {
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo(dir.path());
        crate::worktree::ensure_excluded(&repo).unwrap();
        let excluded = || std::fs::read_to_string(repo.join(".git/info/exclude")).unwrap()
            .lines().any(|line| line == ".codex/hooks.json");
        assert!(!excluded(), "a project's own Codex hooks file is ignored before Codex ever ran");

        super::install_codex(&repo, Instance::Test, Path::new("/opt/relay")).unwrap();
        assert!(excluded());
        let status = Command::new("git").arg("-C").arg(&repo).args(["status", "--porcelain", "--untracked-files=all"]).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&status.stdout), "");

        std::fs::create_dir(dir.path().join("tracked")).unwrap();
        let tracked = init_repo(&dir.path().join("tracked"));
        let project = r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"true"}]}]}}"#;
        std::fs::create_dir_all(tracked.join(".codex")).unwrap();
        std::fs::write(tracked.join(".codex/hooks.json"), project).unwrap();
        let add = Command::new("git").arg("-C").arg(&tracked).args(["add", ".codex/hooks.json"]).status().unwrap();
        assert!(add.success());
        let refused = super::install_codex(&tracked, Instance::Test, Path::new("/opt/relay")).unwrap_err().to_string();
        assert!(refused.contains("tracked by git"), "{refused}");
        assert_eq!(std::fs::read_to_string(tracked.join(".codex/hooks.json")).unwrap(), project);
    }

    /// Every file Relay writes into a checkout goes through a temp file opened without
    /// following links, and a linked directory on the way is replaced by a real one.
    #[test]
    fn adapter_writes_never_follow_a_planted_symlink() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let repo = init_repo(dir.path());
        let outside = dir.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        let victim = outside.join("victim");
        std::fs::write(&victim, "precious\n").unwrap();

        symlink(&outside, repo.join(".claude")).unwrap();
        std::fs::create_dir_all(repo.join(".codex")).unwrap();
        symlink(&victim, repo.join(".codex/hooks.json.relay-tmp")).unwrap();
        std::fs::create_dir_all(repo.join(".relay/hooks/calm-otter")).unwrap();
        symlink(&victim, repo.join(".relay/hooks/calm-otter/pre-commit")).unwrap();

        install_git(&repo, &repo, "calm-otter", Instance::Test, Path::new("/bin/true")).unwrap();
        install_claude(&repo, Instance::Test, Path::new("/opt/relay")).unwrap();
        super::install_codex(&repo, Instance::Test, Path::new("/opt/relay")).unwrap();

        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "precious\n");
        assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 1, "something was written through the link");
        for path in [".claude", ".relay/hooks/calm-otter/pre-commit", ".codex/hooks.json"] {
            assert!(!std::fs::symlink_metadata(repo.join(path)).unwrap().file_type().is_symlink(), "{path} is still a link");
        }
        assert!(repo.join(".claude/settings.local.json").is_file());
        let mode = std::fs::metadata(repo.join(".relay/hooks/calm-otter/pre-commit")).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111, "the hook is not executable");
    }

    /// A FIFO, a link or an oversized file planted where Relay reads a record or an adapter is
    /// refused at once, never waited on: the sweep runs under the store mutex before the socket
    /// binds, and the adapters under the global hook-writes lock.
    #[test]
    fn planted_fifos_links_and_huge_files_are_refused_without_blocking() {
        fn mkfifo(path: &Path) {
            let name = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
            assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o644) }, 0, "mkfifo {}", path.display());
        }
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().to_path_buf();
        let stale = repo.join(".relay/hooks/dead-koala");
        std::fs::create_dir_all(&stale).unwrap();
        mkfifo(&stale.join(super::HOOK_OWNER));
        mkfifo(&stale.join(super::PREVIOUS_HOOKS_PATH));
        let linked = repo.join(".relay/hooks/linked-mole");
        std::fs::create_dir_all(&linked).unwrap();
        std::fs::write(repo.join("elsewhere"), "test\n").unwrap();
        std::os::unix::fs::symlink(repo.join("elsewhere"), linked.join(super::HOOK_OWNER)).unwrap();
        std::os::unix::fs::symlink(repo.join("elsewhere"), linked.join(super::PREVIOUS_HOOKS_PATH)).unwrap();
        std::fs::create_dir_all(repo.join(".claude")).unwrap();
        mkfifo(&repo.join(".claude/settings.local.json"));
        std::fs::create_dir_all(repo.join(".codex")).unwrap();
        std::fs::write(repo.join(".codex/hooks.json"), vec![b' '; (crate::private::MAX_READ + 1) as usize]).unwrap();

        let (done, finished) = std::sync::mpsc::channel();
        let worker = repo.clone();
        std::thread::spawn(move || {
            let swept = sweep_hook_dirs(&worker, &[], Instance::Test);
            let records = (super::read_record(&worker.join(".relay/hooks/dead-koala")),
                super::read_record(&worker.join(".relay/hooks/linked-mole")));
            let claude = (install_claude(&worker, Instance::Test, Path::new("/opt/relay")).is_err(),
                super::uninstall_claude(&worker).is_err());
            let codex = super::uninstall_codex(&worker).is_err();
            done.send((swept, records, claude, codex)).unwrap();
        });
        let (swept, records, claude, codex) = finished.recv_timeout(std::time::Duration::from_secs(10))
            .expect("a read blocked on a planted FIFO");
        assert_eq!(swept, 0, "an unreadable owner is not this instance's");
        assert!(stale.is_dir() && linked.is_dir());
        assert_eq!(records, (None, None));
        assert_eq!(claude, (true, true), "a FIFO is not Claude's settings");
        assert!(codex, "a file past the cap was parsed");
    }
}
