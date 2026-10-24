//! Device leases: one holder per Android device at a time.
//!
//! Several agent sessions working on one app used to install over each other on the same phone —
//! one `gradlew installDebug` replacing the APK another session was halfway through testing, with
//! neither knowing the other existed. A lease makes the phone's current user visible and turns
//! the clobber into a typed refusal (`device.busy`) that names the holder, what it is doing and
//! since when, and says how to wait (`bus.wait` on `device.lease.released`).
//!
//! Three things take a lease:
//! - `device.run` — for as long as the run is live (building, installing, streaming logcat);
//! - an agent's own device-writing shell command (`adb install`, `adb shell am start`,
//!   `gradlew installDebug`, …), seen by the guardrail hook before it runs, and kept for a short
//!   grace period after it finishes so the agent can look at what it installed;
//! - `device.claim`, an explicit hold across several steps.
//!
//! Leases live in memory: a restart is a release, which is exactly right for a crash. A lease
//! also lapses with its run, its session's exit or close, or its own expiry. The map sits behind
//! its own small mutex and never touches SQLite, so taking one costs nothing in the store lock.
//!
//! One holder can hold several leases on one device — a claim, and the run it then starts — and
//! the strongest is the one others see. Ending the run leaves the claim in place.

use relay_bus::error::BusError;
use relay_bus::types::{DeviceLease, Id};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// The key of a lease whose command named no device. `adb` without `-s` uses the only device
/// and Gradle's `install*` tasks install on every device, so it conflicts with all of them.
pub const ANY_DEVICE: &str = "*";
/// How long a shell lease lasts while its command may still be running. Claude's Bash tool
/// gives up on a command after ten minutes; the hook that ends the command shortens this.
pub const SHELL_RUNNING: Duration = Duration::from_secs(15 * 60);
/// How long a shell lease outlives the command that took it: enough to look at the screen or
/// the log of what was just installed before someone else installs over it.
pub const SHELL_GRACE: Duration = Duration::from_secs(90);
pub const CLAIM_DEFAULT_MINUTES: u32 = 10;
pub const CLAIM_MAX_MINUTES: u32 = 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Holder {
    Session { id: Id, name: String },
    User,
}

impl Holder {
    pub fn session_id(&self) -> Option<Id> {
        match self {
            Holder::Session { id, .. } => Some(*id),
            Holder::User => None,
        }
    }
    fn describe(&self) -> String {
        match self {
            Holder::Session { name, .. } => format!("session {name}"),
            Holder::User => "you (the Relay user)".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Lasts exactly as long as the `device.run` it belongs to.
    Run(Id),
    Claim,
    Shell,
}

impl Kind {
    fn rank(self) -> u8 {
        match self {
            Kind::Run(_) => 2,
            Kind::Claim => 1,
            Kind::Shell => 0,
        }
    }
    fn as_str(self) -> &'static str {
        match self {
            Kind::Run(_) => "run",
            Kind::Claim => "claim",
            Kind::Shell => "shell",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Lease {
    pub device: String,
    pub holder: Holder,
    pub kind: Kind,
    pub action: String,
    pub since: String,
    started: Instant,
    expires: Option<Instant>,
    /// Shell commands seen starting under this lease and not yet seen finishing. Parallel tool
    /// calls each take the lease; it shrinks to the grace period only when the last one ends.
    commands: u32,
    /// A command under it went to the background, so its end is never reported: the lease then
    /// runs out its full [`SHELL_RUNNING`] term instead of shrinking to the grace period.
    background: bool,
}

impl Lease {
    pub fn new(device: &str, holder: Holder, kind: Kind, action: &str, ttl: Option<Duration>) -> Lease {
        let now = Instant::now();
        Lease {
            device: if device.trim().is_empty() { ANY_DEVICE.into() } else { device.trim().to_string() },
            holder,
            kind,
            action: clip(action),
            since: crate::time::now(),
            started: now,
            expires: ttl.map(|ttl| now + ttl),
            commands: u32::from(kind == Kind::Shell),
            background: false,
        }
    }

    pub fn view(&self) -> DeviceLease {
        DeviceLease {
            device: self.device.clone(),
            session: match &self.holder {
                Holder::Session { name, .. } => Some(name.clone()),
                Holder::User => None,
            },
            kind: self.kind.as_str().into(),
            action: self.action.clone(),
            since: self.since.clone(),
            run_id: match self.kind { Kind::Run(id) => Some(id), _ => None },
            expires_in_s: self.expires.map(|at| at.saturating_duration_since(Instant::now()).as_secs()),
        }
    }

    /// The payload of `device.lease.acquired` / `device.lease.released`.
    pub fn event(&self) -> Value {
        serde_json::to_value(self.view()).unwrap_or_else(|_| json!({"device": self.device}))
    }

    fn overlaps(&self, device: &str) -> bool {
        self.device == device || self.device == ANY_DEVICE || device == ANY_DEVICE
    }
}

/// The refusal a conflicting caller gets: who, doing what, since when, and how to wait.
pub fn busy(lease: &Lease) -> BusError {
    let device = if lease.device == ANY_DEVICE { "every connected device".to_string() } else { format!("device {}", lease.device) };
    let mut message = format!(
        "{device} is in use by {}: {} (since {}, {} ago)",
        lease.holder.describe(), lease.action, clock(&lease.since), ago(lease.started.elapsed()),
    );
    match lease.kind {
        Kind::Run(id) => message.push_str(&format!("; it is device run {id}, which holds the device until it stops")),
        _ => if let Some(at) = lease.expires {
            message.push_str(&format!("; the lease lapses in {} unless renewed", ago(at.saturating_duration_since(Instant::now()))));
        },
    }
    let mut hint = String::from(
        "wait for it with bus.wait {\"events\":[\"device.lease.released\"]} and retry; device.list and device.leases show every holder",
    );
    if let Holder::Session { name, .. } = &lease.holder {
        hint.push_str(&format!("; to coordinate, mailbox.send {{\"to\":\"{name}\",...}}"));
    }
    BusError::conflict("device.busy", message)
        .with_hint(hint)
        .with_details(json!({ "lease": lease.event() }))
}

/// A lease acquisition made by a request that has not committed yet. The lease map lives
/// outside the store, so a request that rolled back — a later step refused, or the commit
/// itself failed — used to leave the lease held while its `device.lease.acquired` event went
/// with the rest of the request's events: the device was busy for up to [`SHELL_RUNNING`] under
/// a lease nobody had been told of (RA-358). Dropped without [`Pending::keep`], it takes the
/// acquisition back, silently, since nothing was announced: the lease map and the event stream
/// both stay as if the request had never run.
pub struct Pending {
    /// `None` once kept.
    leases: Option<Leases>,
    device: String,
    holder: Holder,
    kind: Kind,
    commands: u32,
    /// For a renewal, the action and expiry the lease had before it.
    prior: Option<(String, Option<Instant>)>,
}

impl Pending {
    /// The request committed: the acquisition stands.
    pub fn keep(mut self) {
        self.leases = None;
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        if let Some(leases) = self.leases.take() {
            leases.revert(self);
        }
    }
}

/// Every holder's leases on one device key, in the order they were taken. All share a holder:
/// another holder is refused before it can add one.
type Stack = Vec<Lease>;

/// The lease others see on a device: the strongest unexpired one, the latest among equals.
fn visible(stack: &[Lease], now: Instant) -> Option<&Lease> {
    stack.iter().filter(|held| held.expires.is_none_or(|at| at > now)).max_by_key(|held| held.kind.rank())
}

/// The lease map. A clone is a handle to the same map, which is how the expiry sweeper waits
/// on it without holding the engine.
#[derive(Default, Clone)]
pub struct Leases {
    shared: Arc<Shared>,
}

#[derive(Default)]
struct Shared {
    map: Mutex<HashMap<String, Stack>>,
    /// Signalled whenever a lease is added or its expiry moves, so the expiry sweeper re-reads
    /// the map instead of sleeping past the new deadline.
    changed: Condvar,
    sweeping: AtomicBool,
}

impl Leases {
    fn map(&self) -> std::sync::MutexGuard<'_, HashMap<String, Stack>> {
        self.shared.map.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    /// Take (or renew) a lease on `lease.device`. Refuses with [`busy`] when someone else holds
    /// that device, or holds every device, or — for `*` — holds any device at all. The same
    /// holder never conflicts with itself, and a holder's leases of different kinds stack: a
    /// shell command during the holder's own run leaves the run's lease in front, and a run
    /// started under a claim hands the device back to the claim when it ends.
    ///
    /// Returns whether the lease is new and now the one others see (the caller emits
    /// `device.lease.acquired`).
    pub fn acquire(&self, lease: Lease, pruned: &mut Vec<Lease>) -> Result<bool, BusError> {
        self.acquire_pending(lease, pruned).map(|(taken, pending)| {
            pending.keep();
            taken
        })
    }

    /// [`Leases::acquire`] for a request that has yet to commit: the [`Pending`] it returns
    /// gives the acquisition back when dropped, unless it is kept.
    pub fn acquire_pending(&self, lease: Lease, pruned: &mut Vec<Lease>) -> Result<(bool, Pending), BusError> {
        let mut map = self.map();
        pruned.extend(expire(&mut map));
        if let Some(other) = map.values().flatten().filter(|held| held.overlaps(&lease.device) && held.holder != lease.holder)
            .min_by_key(|held| held.started)
        {
            return Err(busy(other));
        }
        let mut pending = Pending {
            leases: Some(self.clone()),
            device: lease.device.clone(),
            holder: lease.holder.clone(),
            kind: lease.kind,
            commands: lease.commands,
            prior: None,
        };
        let stack = map.entry(lease.device.clone()).or_default();
        let taken = if let Some(held) = stack.iter_mut().find(|held| held.kind == lease.kind) {
            // A renewal: keep when it started, move what it is doing and when it lapses.
            pending.prior = Some((held.action.clone(), held.expires));
            held.action = lease.action;
            held.expires = match (held.expires, lease.expires) {
                (Some(a), Some(b)) => Some(a.max(b)),
                _ => None,
            };
            held.commands += lease.commands;
            false
        } else {
            let outranked = stack.iter().any(|held| held.kind.rank() > lease.kind.rank());
            stack.push(lease);
            !outranked
        };
        self.shared.changed.notify_all();
        Ok((taken, pending))
    }

    /// Undo one acquisition (see [`Pending`]): a lease it added goes, a renewal gets back the
    /// action and expiry it had and gives back the command it counted.
    fn revert(&self, pending: &Pending) {
        let mut map = self.map();
        if let Some(stack) = map.get_mut(&pending.device) {
            if let Some(at) = stack.iter().position(|held| held.kind == pending.kind && held.holder == pending.holder) {
                match &pending.prior {
                    None => { stack.remove(at); }
                    Some((action, expires)) => {
                        let held = &mut stack[at];
                        held.action = action.clone();
                        held.expires = *expires;
                        held.commands = held.commands.saturating_sub(pending.commands);
                    }
                }
            }
        }
        map.retain(|_, stack| !stack.is_empty());
        self.shared.changed.notify_all();
    }

    /// Would `holder` be refused on `device`? Never takes anything.
    pub fn check(&self, device: &str, holder: &Holder) -> Result<(), BusError> {
        let map = self.map();
        let now = Instant::now();
        match map.values().flatten().filter(|held| held.overlaps(device) && &held.holder != holder)
            .filter(|held| held.expires.is_none_or(|at| at > now))
            .min_by_key(|held| held.started)
        {
            Some(other) => Err(busy(other)),
            None => Ok(()),
        }
    }

    /// The lease others see on each held device.
    pub fn list(&self) -> Vec<Lease> {
        let map = self.map();
        let now = Instant::now();
        let mut out: Vec<Lease> = map.values().filter_map(|stack| visible(stack, now)).cloned().collect();
        out.sort_by(|a, b| a.device.cmp(&b.device));
        out
    }

    /// The lease that covers `device` (its own, or a `*` one), for `device.list`.
    pub fn holder_of(&self, device: &str) -> Option<Lease> {
        let now = Instant::now();
        let map = self.map();
        map.get(device).and_then(|stack| visible(stack, now))
            .or_else(|| map.get(ANY_DEVICE).and_then(|stack| visible(stack, now)))
            .cloned()
    }

    /// Drop every lease the predicate selects; return what went, for the release events.
    pub fn release_where(&self, mut select: impl FnMut(&Lease) -> bool) -> Vec<Lease> {
        let mut map = self.map();
        let mut gone = expire(&mut map);
        for stack in map.values_mut() {
            let mut index = 0;
            while index < stack.len() {
                if select(&stack[index]) { gone.push(stack.remove(index)); } else { index += 1; }
            }
        }
        map.retain(|_, stack| !stack.is_empty());
        gone
    }

    pub fn release_run(&self, run_id: Id) -> Vec<Lease> {
        self.release_where(|lease| lease.kind == Kind::Run(run_id))
    }

    /// A run stopped while it may still be installing: Gradle's daemon outlives the stop and can
    /// go on with `adb install` for a while. Its lease becomes a shell lease of the same holder
    /// for `grace` — the holder's own shell lease on the device, lengthened, when it has one —
    /// rather than leaving the device open for someone else's install to collide with that one.
    /// Returns the run's lease that went, and the grace lease when it is the one others now see
    /// (for `device.lease.released` and `device.lease.acquired`).
    pub fn stop_run(&self, run_id: Id, grace: Duration) -> (Vec<Lease>, Option<Lease>) {
        let mut map = self.map();
        let mut gone = expire(&mut map);
        let now = Instant::now();
        let mut shown = None;
        for stack in map.values_mut() {
            let Some(at) = stack.iter().position(|held| held.kind == Kind::Run(run_id)) else { continue };
            let run = stack.remove(at);
            let until = now + grace;
            if let Some(shell) = stack.iter_mut().find(|held| held.kind == Kind::Shell) {
                shell.expires = Some(shell.expires.map_or(until, |at| at.max(until)));
            } else {
                let mut lease = Lease::new(&run.device, run.holder.clone(), Kind::Shell, &format!("{} (stopping)", run.action), Some(grace));
                lease.commands = 0;
                stack.push(lease);
            }
            shown = visible(stack, now).filter(|lease| lease.kind == Kind::Shell).cloned();
            gone.push(run);
        }
        self.shared.changed.notify_all();
        (gone, shown)
    }

    /// A session that exits or closes gives back its shell leases and claims. A run it asked
    /// for stays with the run: the run, not the session, is what is using the device.
    pub fn release_session(&self, session_id: Id) -> Vec<Lease> {
        self.release_where(|lease| lease.holder.session_id() == Some(session_id) && !matches!(lease.kind, Kind::Run(_)))
    }

    /// One shell command that took the session's lease on `device` has finished. The lease keeps
    /// only the grace period once none of its commands is still running — and keeps its full
    /// term if one of them went to the background, whose end nobody reports.
    pub fn shell_command_finished(&self, session_id: Id, device: &str, background: bool) {
        let mut map = self.map();
        let Some(stack) = map.get_mut(device) else { return };
        for lease in stack.iter_mut().filter(|lease| lease.kind == Kind::Shell && lease.holder.session_id() == Some(session_id)) {
            lease.commands = lease.commands.saturating_sub(1);
            lease.background |= background;
            if lease.commands == 0 { shrink(lease); }
        }
        self.shared.changed.notify_all();
    }

    /// The session's turn ended: no foreground command of it is still running, so every shell
    /// lease it holds keeps only the grace period (unless a background command holds it).
    pub fn shell_turn_ended(&self, session_id: Id) {
        let mut map = self.map();
        for lease in map.values_mut().flatten().filter(|lease| lease.kind == Kind::Shell && lease.holder.session_id() == Some(session_id)) {
            lease.commands = 0;
            shrink(lease);
        }
        self.shared.changed.notify_all();
    }

    /// Drop what has lapsed, plus anything `stale` says is held by something that is gone (a
    /// run no longer live, a session no longer running). Returns what went.
    pub fn prune(&self, mut stale: impl FnMut(&Lease) -> bool) -> Vec<Lease> {
        self.release_where(|lease| stale(lease))
    }

    pub fn is_empty(&self) -> bool {
        self.map().is_empty()
    }

    /// Become the expiry sweeper. True for exactly one caller until [`Leases::wait_expired`]
    /// finds the map empty and stands the sweeper down.
    pub fn start_sweeper(&self) -> bool {
        !self.shared.sweeping.swap(true, Ordering::SeqCst)
    }

    /// The sweeper's step: sleep until the next lease lapses (at most `cap`, or until a lease
    /// changes), then drop and return what lapsed, for its `device.lease.released`. `None` once
    /// nothing is held: the sweeper stops, and the next lease taken starts a new one.
    pub fn wait_expired(&self, cap: Duration) -> Option<Vec<Lease>> {
        let mut map = self.map();
        if map.is_empty() {
            self.shared.sweeping.store(false, Ordering::SeqCst);
            return None;
        }
        let now = Instant::now();
        let next = map.values().flatten().filter_map(|lease| lease.expires).min();
        let wait = next.map_or(cap, |at| at.saturating_duration_since(now).min(cap));
        if !wait.is_zero() {
            map = self.shared.changed.wait_timeout(map, wait).unwrap_or_else(|poison| poison.into_inner()).0;
        }
        Some(expire(&mut map))
    }
}

/// A finished shell command's lease keeps only [`SHELL_GRACE`] — unless a background command
/// may still be using the device.
fn shrink(lease: &mut Lease) {
    if lease.background { return; }
    let until = Instant::now() + SHELL_GRACE;
    lease.expires = Some(lease.expires.map_or(until, |at| at.min(until)));
}

fn expire(map: &mut HashMap<String, Stack>) -> Vec<Lease> {
    let now = Instant::now();
    let mut gone = Vec::new();
    for stack in map.values_mut() {
        let mut index = 0;
        while index < stack.len() {
            if stack[index].expires.is_some_and(|at| at <= now) { gone.push(stack.remove(index)); } else { index += 1; }
        }
    }
    map.retain(|_, stack| !stack.is_empty());
    gone
}

fn clip(action: &str) -> String {
    let one_line = action.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= 120 { return one_line; }
    let mut cut: String = one_line.chars().take(119).collect();
    cut.push('…');
    cut
}

/// `2026-10-05T14:02:11.123Z` → `14:02:11 UTC`. Anything else is shown as given.
fn clock(ts: &str) -> String {
    ts.split_once('T')
        .map(|(_, time)| format!("{} UTC", time.get(..8).unwrap_or(time)))
        .unwrap_or_else(|| ts.to_string())
}

fn ago(elapsed: Duration) -> String {
    crate::time::span(elapsed.as_secs(), crate::time::Unit::Second)
}

/// What an agent's shell command does to a device, when it does anything that could clobber
/// another session's work there: installs, uninstalls, launches, input, pushes, reboots. Reads
/// (`logcat`, `devices`, `getprop`, `dumpsys`, `screencap`) are free for everyone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceCommand {
    /// The serial it targets, or [`ANY_DEVICE`].
    pub device: String,
    /// A few words for the lease (`adb install app-debug.apk`).
    pub action: String,
}

/// Recognize device-writing commands in a shell line. Conservative by design: it splits on the
/// shell's sequencing operators and looks at each simple command's program and verb; it does not
/// try to be a shell. Whatever it misses runs unleased, as everything did before.
pub fn device_command(line: &str) -> Option<DeviceCommand> {
    let mut exported_serial: Option<String> = None;
    // The same reader the denied-command guardrail uses: `( )`, `$( )`, backticks and
    // `sh -c '…'` each start a command, and a quoted argument stays one word.
    for segment in crate::shell::commands(line) {
        let tokens: Vec<String> = segment.into_iter().map(|word| word.text).collect();
        let mut index = 0;
        let mut serial = exported_serial.clone();
        // Leading assignments and wrappers: `ANDROID_SERIAL=x env timeout 300 ./gradlew …`.
        while index < tokens.len() {
            let token = tokens[index].as_str();
            if let Some(value) = token.strip_prefix("ANDROID_SERIAL=") {
                serial = Some(value.to_string());
                index += 1;
            } else if token == "export" {
                if let Some(value) = tokens.get(index + 1).and_then(|t| t.strip_prefix("ANDROID_SERIAL=")) {
                    exported_serial = Some(value.to_string());
                }
                break;
            } else if is_assignment(token) || matches!(token, "env" | "sudo" | "time" | "nice" | "nohup" | "command" | "exec" | "stdbuf") {
                index += 1;
            } else if token == "timeout" {
                index += 1;
                while tokens.get(index).is_some_and(|t| t.starts_with('-')) { index += 1; }
                index += 1; // the duration
            } else if token == "cd" || token == "pushd" {
                index = tokens.len();
            } else {
                break;
            }
        }
        let Some(program) = tokens.get(index) else { continue };
        let args = &tokens[index + 1..];
        let base = program.rsplit('/').next().unwrap_or(program);
        let found = match base {
            "adb" | "adb.exe" => adb_command(args, serial.as_deref()),
            "gradlew" | "gradlew.bat" | "gradle" => gradle_command(base, args, serial.as_deref()),
            "npx" | "bunx" | "pnpx" | "yarn" | "pnpm" | "npm" | "expo" | "react-native" | "flutter" => {
                js_command(base, args, serial.as_deref())
            }
            _ => None,
        };
        if found.is_some() {
            return found;
        }
    }
    None
}

fn is_assignment(token: &str) -> bool {
    token.split_once('=').is_some_and(|(name, _)| {
        !name.is_empty() && name.chars().all(|c| c == '_' || c.is_ascii_alphanumeric())
            && !name.starts_with(|c: char| c.is_ascii_digit())
    })
}

/// Shell verbs (after `adb shell`) that change what is on the device or on its screen.
const SHELL_WRITES: &[&str] = &["am", "pm", "cmd", "input", "monkey", "reboot", "svc", "settings", "wm", "setprop", "rm", "mv", "cp", "uiautomator", "su"];
/// `pm` / `cmd package` verbs that only read.
const PM_READS: &[&str] = &["list", "path", "dump", "resolve-activity", "query-activities", "get-max-users", "has-feature", "help"];
/// `am` / `cmd activity` verbs that only read.
const AM_READS: &[&str] = &["get-config", "get-current-user", "stack", "task", "monitor", "dumpheap", "profile", "help"];
/// Verbs of any other `cmd <service>` that only read (`cmd overlay list`, `cmd wifi status`,
/// `cmd settings get …`). Every other service command is taken for a write.
const CMD_READS: &[&str] = &["list", "status", "get", "help", "dump", "query"];
/// Programs the device's shell runs another program under: `toybox rm`, `nohup am start`.
const SHELL_WRAPPERS: &[&str] = &["env", "exec", "nohup", "nice", "toybox", "busybox", "command", "time"];
/// `wm` verbs that print the current value when given none, and set it when given one.
const WM_QUERIES: &[&str] = &["size", "density", "user-rotation", "fixed-to-user-rotation", "scaling"];

/// Does one command run by `adb shell` change the device? `words` is the command and its
/// arguments.
fn shell_writes(words: &[&str]) -> bool {
    // `FOO=1 nohup /system/bin/am start …` is `am start …`.
    let start = words.iter().position(|word| !is_assignment(word) && !SHELL_WRAPPERS.contains(&crate::shell::program(word)));
    let words = &words[start.unwrap_or(words.len())..];
    let command = words.first().map_or("", |word| crate::shell::program(word));
    let sub = words.get(1).copied().unwrap_or("");
    let third = words.get(2).copied().unwrap_or("");
    match command {
        "pm" => !PM_READS.contains(&sub),
        "cmd" => match sub {
            "package" => !PM_READS.contains(&third),
            "activity" => !AM_READS.contains(&third),
            // `cmd` alone and `cmd -l` list the services.
            "" | "-l" => false,
            _ => !CMD_READS.iter().any(|read| third == *read || third.starts_with(&format!("{read}-"))),
        },
        "am" => !AM_READS.contains(&sub),
        "settings" => !matches!(sub, "" | "get" | "list" | "help"),
        "wm" => {
            if sub.is_empty() || sub == "help" { return false; }
            if !WM_QUERIES.contains(&sub) { return true; }
            // `wm size -d 1` still only asks, about display 1; any other argument sets.
            let mut args = words[2..].iter();
            while let Some(arg) = args.next() {
                if *arg == "-d" { args.next(); } else { return true; }
            }
            false
        }
        other => SHELL_WRITES.contains(&other),
    }
}

fn adb_command(args: &[String], serial: Option<&str>) -> Option<DeviceCommand> {
    let mut serial = serial.map(str::to_string);
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "-s" | "-t" => {
                if args[index].as_str() == "-s" { serial = args.get(index + 1).cloned(); }
                index += 2;
            }
            "-H" | "-P" | "-L" => index += 2,
            flag if flag.starts_with('-') => index += 1,
            // `adb wait-for-device install …` (and wait-for-usb-…, -recovery, -sideload…) waits,
            // then runs the rest: the command is what follows.
            wait if wait.starts_with("wait-for-") && index + 1 < args.len() => index += 1,
            _ => break,
        }
    }
    let verb = args.get(index)?.as_str();
    let rest = &args[index + 1..];
    let writes = match verb {
        "install" | "install-multiple" | "install-multi-package" | "uninstall" | "push" | "sync" | "reboot" | "root"
        | "unroot" | "remount" | "sideload" | "disable-verity" | "enable-verity" | "restore" | "emu" => true,
        "shell" | "exec-out" => {
            // adb joins its arguments into one line for the device's shell, which reads it again:
            // `adb shell "am start …; input tap 1 1"` and `adb shell "sh -c 'am start …'"` are
            // read the way that shell will, every command of it, nested `sh -c` included.
            let mut words = Vec::new();
            let mut args = rest.iter().map(String::as_str).skip_while(|word| word.starts_with('-'));
            while let Some(word) = args.next() {
                // The local shell's redirections (`> shot.png`, `< steps.sh`) are not adb's.
                if is_redirection(word) {
                    if word.trim_start_matches(|c: char| c.is_ascii_digit() || matches!(c, '<' | '>' | '&')).is_empty() {
                        args.next();
                    }
                } else {
                    words.push(word);
                }
            }
            // No command at all — an interactive shell, or commands fed on stdin (`adb shell <
            // steps.sh`, `echo "am start …" | adb shell`) — can do anything: a write.
            words.is_empty() || crate::shell::commands(&words.join(" ")).iter().any(|command| {
                let words: Vec<&str> = command.iter().map(|word| word.text.as_str()).collect();
                shell_writes(&words)
            })
        }
        _ => false,
    };
    if !writes {
        return None;
    }
    let summary = std::iter::once("adb").chain(std::iter::once(verb))
        .chain(rest.iter().map(String::as_str).take(3).map(|word| word.rsplit('/').next().unwrap_or(word)))
        .collect::<Vec<_>>().join(" ");
    Some(DeviceCommand { device: serial.unwrap_or_else(|| ANY_DEVICE.into()), action: summary })
}

/// A redirection word as the shell reader leaves it: `<`, `>`, `2>&1`, `<steps.sh`, `&>log`.
fn is_redirection(word: &str) -> bool {
    word.trim_start_matches(|c: char| c.is_ascii_digit()).starts_with(['<', '>']) || word.starts_with("&>")
}

fn gradle_command(program: &str, args: &[String], serial: Option<&str>) -> Option<DeviceCommand> {
    let tasks: Vec<&str> = args.iter().map(String::as_str).filter(|arg| !arg.starts_with('-')).filter(|task| {
        let name = task.rsplit(':').next().unwrap_or(task);
        (name.starts_with("install") && name.len() > "install".len())
            || name.starts_with("uninstall")
            || name.starts_with("connected")
            || name.starts_with("deviceAndroidTest")
    }).collect();
    if tasks.is_empty() {
        return None;
    }
    Some(DeviceCommand {
        device: serial.map(str::to_string).unwrap_or_else(|| ANY_DEVICE.into()),
        action: format!("{program} {}", tasks.join(" ")),
    })
}

fn js_command(program: &str, args: &[String], serial: Option<&str>) -> Option<DeviceCommand> {
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    let android = words.iter().any(|word| matches!(*word, "run:android" | "run-android"))
        || (program == "flutter" && matches!(words.first(), Some(&"run" | &"install")));
    if !android {
        return None;
    }
    let device = words.iter().position(|word| matches!(*word, "--device" | "--deviceId" | "-d"))
        .and_then(|at| words.get(at + 1))
        .map(|value| value.to_string())
        .or_else(|| serial.map(str::to_string))
        .unwrap_or_else(|| ANY_DEVICE.into());
    let shown: Vec<&str> = std::iter::once(program).chain(words.iter().copied().take(3)).collect();
    Some(DeviceCommand { device, action: shown.join(" ") })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(line: &str) -> Option<(String, String)> {
        device_command(line).map(|found| (found.device, found.action))
    }

    /// RA-358: an acquisition whose request rolled back is taken back, and a renewal gets back
    /// what it had; one whose request committed stands.
    #[test]
    fn an_acquisition_that_does_not_commit_is_taken_back() {
        let leases = Leases::default();
        let me = Holder::Session { id: 1, name: "me".into() };
        let shell = |action: &str| Lease::new("phone", me.clone(), Kind::Shell, action, Some(SHELL_RUNNING));

        let (taken, pending) = leases.acquire_pending(shell("adb install"), &mut Vec::new()).unwrap();
        assert!(taken);
        drop(pending);
        assert!(leases.list().is_empty(), "a rolled-back request left its lease held");

        let (_, pending) = leases.acquire_pending(shell("adb install"), &mut Vec::new()).unwrap();
        pending.keep();
        leases.shell_command_finished(1, "phone", false);
        let before = leases.holder_of("phone").unwrap();
        assert_eq!(before.commands, 0);
        let (taken, pending) = leases.acquire_pending(shell("adb shell input tap 1 1"), &mut Vec::new()).unwrap();
        assert!(!taken, "a renewal");
        assert_eq!(leases.holder_of("phone").unwrap().commands, 1);
        drop(pending);
        let after = leases.holder_of("phone").unwrap();
        assert_eq!((after.commands, after.action.as_str(), after.expires), (0, "adb install", before.expires));

        // Someone else is refused while the kept lease stands.
        let other = Lease::new("phone", Holder::User, Kind::Claim, "testing", None);
        assert!(leases.acquire_pending(other, &mut Vec::new()).is_err());
    }

    #[test]
    fn device_writes_are_recognized_and_reads_are_not() {
        assert_eq!(device("adb install -r app/build/outputs/apk/debug/app-debug.apk").unwrap().0, ANY_DEVICE);
        assert_eq!(device("adb -s R5CT123 install -r app-debug.apk").unwrap(), ("R5CT123".into(), "adb install -r app-debug.apk".into()));
        assert_eq!(device("cd android && ./gradlew :app:installDebug --offline").unwrap(), (ANY_DEVICE.into(), "gradlew :app:installDebug".into()));
        assert_eq!(device("ANDROID_SERIAL=emulator-5554 ./gradlew installRelease").unwrap().0, "emulator-5554");
        assert_eq!(device("export ANDROID_SERIAL=abc; ./gradlew installDebug").unwrap().0, "abc");
        assert!(device("adb shell am start -n com.example/.Main").is_some());
        assert!(device("adb -s X shell 'am force-stop com.example'").is_some());
        assert!(device("adb shell input tap 100 200").is_some());
        assert!(device("adb shell pm clear com.example").is_some());
        assert!(device("adb shell wm size 1080x1920").is_some());
        assert!(device("adb shell wm size reset").is_some());
        assert!(device("adb shell settings put global animator_duration_scale 0").is_some());
        assert!(device("adb shell cmd package install-existing com.example").is_some());
        assert!(device("adb shell 'wm size; input keyevent 3'").is_some());
        assert!(device("timeout 600 npx expo run:android --device R5CT").unwrap().0 == "R5CT");
        assert!(device("adb uninstall com.example").is_some());
        // Waiting for the device first changes nothing about what runs after.
        assert_eq!(device("adb wait-for-device install app.apk").unwrap(), (ANY_DEVICE.into(), "adb install app.apk".into()));
        assert_eq!(device("adb -s R5CT wait-for-usb-device shell am start -n x/.Main").unwrap().0, "R5CT");
        assert!(device("adb wait-for-device").is_none(), "only waiting, nothing to lease");
        assert!(device("adb wait-for-device shell getprop ro.build.version.sdk").is_none());
        // Any service's command writes unless it is a known read; a program named by its path
        // is that program; `sh -c` on the device is read inside; no command at all is a shell
        // fed on stdin (or a person's), which can do anything.
        assert!(device("adb shell cmd statusbar expand-notifications").is_some());
        assert!(device("adb shell cmd connectivity airplane-mode enable").is_some());
        assert!(device("adb shell cmd settings put global animator_duration_scale 0").is_some());
        assert!(device("adb shell /system/bin/am start -n com.example/.Main").is_some());
        assert!(device("adb shell /system/bin/cmd overlay enable com.x").is_some());
        assert!(device("adb shell \"sh -c 'am start -n com.example/.Main'\"").is_some());
        assert!(device("adb shell 'toybox rm -r /sdcard/x'").is_some());
        assert!(device("adb shell 'FOO=1 nohup input tap 1 1'").is_some());
        assert!(device("adb shell").is_some());
        assert!(device("adb -s R5 shell -t").is_some());
        assert!(device("adb shell < steps.sh").is_some());
        assert!(device("adb shell <steps.sh").is_some());
        assert!(device("echo 'am start -n com.example/.Main' | adb shell").is_some());
        assert!(device("adb shell 2>&1").is_some());
        // Reads are free.
        assert!(device("adb devices -l").is_none());
        assert!(device("adb logcat -d | grep FATAL").is_none());
        assert!(device("adb shell pm list packages | grep example").is_none());
        assert!(device("adb shell getprop ro.build.version.sdk").is_none());
        assert!(device("adb shell wm size").is_none());
        assert!(device("adb shell wm density -d 0").is_none());
        assert!(device("adb -s R5 shell settings get global adb_enabled").is_none());
        assert!(device("adb shell settings list secure").is_none());
        assert!(device("adb shell cmd package list packages -3").is_none());
        assert!(device("adb shell 'cmd activity get-current-user'").is_none());
        assert!(device("adb exec-out screencap -p > shot.png").is_none());
        assert!(device("adb shell cmd overlay list").is_none());
        assert!(device("adb shell cmd wifi status").is_none());
        assert!(device("adb shell cmd settings get global adb_enabled").is_none());
        assert!(device("adb shell cmd -l").is_none());
        assert!(device("adb shell /system/bin/getprop ro.product.model").is_none());
        assert!(device("adb shell \"sh -c 'dumpsys battery'\"").is_none());
        assert!(device("adb shell getprop 2>&1 | head").is_none());
        assert!(device("./gradlew assembleDebug test lint").is_none());
        assert!(device("./gradlew install").is_none());
        assert!(device("cargo test && git commit -m 'adb install fix'").is_none());
        assert!(device("echo adb install").is_none());
    }

    #[test]
    fn a_lease_refuses_other_holders_and_renews_for_its_own() {
        let leases = Leases::default();
        let a = Holder::Session { id: 1, name: "brisk-otter".into() };
        let b = Holder::Session { id: 2, name: "calm-heron".into() };
        let mut pruned = Vec::new();
        assert!(leases.acquire(Lease::new("R5", a.clone(), Kind::Shell, "adb install app.apk", Some(SHELL_RUNNING)), &mut pruned).unwrap());
        assert!(!leases.acquire(Lease::new("R5", a.clone(), Kind::Shell, "adb shell am start", Some(SHELL_RUNNING)), &mut pruned).unwrap());
        let refused = leases.acquire(Lease::new("R5", b.clone(), Kind::Shell, "gradlew installDebug", Some(SHELL_RUNNING)), &mut pruned).unwrap_err();
        assert_eq!(refused.code, "device.busy");
        assert!(refused.message.contains("brisk-otter"), "{}", refused.message);
        assert!(refused.message.contains("adb shell am start"), "{}", refused.message);
        assert!(refused.hint.as_deref().unwrap().contains("device.lease.released"));
        // `*` conflicts with every device, both ways.
        assert_eq!(leases.acquire(Lease::new(ANY_DEVICE, b.clone(), Kind::Shell, "adb install x", None), &mut pruned).unwrap_err().code, "device.busy");
        assert!(leases.acquire(Lease::new("OTHER", b.clone(), Kind::Shell, "adb install x", None), &mut pruned).unwrap());
        // A run outranks the holder's own shell lease and is not downgraded by it.
        assert!(leases.acquire(Lease::new("R5", a.clone(), Kind::Run(7), "device.run", None), &mut pruned).unwrap());
        assert!(!leases.acquire(Lease::new("R5", a.clone(), Kind::Shell, "adb install", Some(SHELL_RUNNING)), &mut pruned).unwrap());
        assert_eq!(leases.holder_of("R5").unwrap().kind, Kind::Run(7));
        // Closing the session gives back its own shell lease and leaves its run's; the run
        // ending releases that.
        assert!(leases.release_session(1).iter().all(|lease| lease.kind == Kind::Shell));
        assert_eq!(leases.holder_of("R5").unwrap().kind, Kind::Run(7));
        assert_eq!(leases.release_run(7).len(), 1);
        assert!(leases.check("R5", &b).is_ok());
    }

    #[test]
    fn a_finished_shell_command_keeps_its_lease_only_for_the_grace_period() {
        let leases = Leases::default();
        let a = Holder::Session { id: 1, name: "a".into() };
        let mut pruned = Vec::new();
        leases.acquire(Lease::new("R5", a.clone(), Kind::Shell, "adb install", Some(SHELL_RUNNING)), &mut pruned).unwrap();
        leases.shell_command_finished(1, "R5", false);
        let left = leases.holder_of("R5").unwrap().view().expires_in_s.unwrap();
        assert!(left <= SHELL_GRACE.as_secs(), "{left}");
        // An already-lapsed lease is dropped by the next acquire and reported for its event.
        let lapsed = Lease::new("R6", a.clone(), Kind::Shell, "adb install", Some(Duration::ZERO));
        leases.acquire(lapsed, &mut pruned).unwrap();
        leases.acquire(Lease::new("R7", Holder::User, Kind::Claim, "testing", None), &mut pruned).unwrap();
        assert!(pruned.iter().any(|lease| lease.device == "R6"));
    }

    #[test]
    fn a_shell_lease_waits_for_every_command_under_it() {
        let leases = Leases::default();
        let a = Holder::Session { id: 1, name: "a".into() };
        let mut pruned = Vec::new();
        let shell = || Lease::new("R5", a.clone(), Kind::Shell, "adb install", Some(SHELL_RUNNING));
        // Two device commands in parallel: the first to finish leaves the lease alone.
        leases.acquire(shell(), &mut pruned).unwrap();
        leases.acquire(shell(), &mut pruned).unwrap();
        leases.shell_command_finished(1, "R5", false);
        assert!(leases.holder_of("R5").unwrap().view().expires_in_s.unwrap() > SHELL_GRACE.as_secs());
        leases.shell_command_finished(1, "R5", false);
        assert!(leases.holder_of("R5").unwrap().view().expires_in_s.unwrap() <= SHELL_GRACE.as_secs());
        // A background command's PostToolUse arrives at once; the lease keeps its full term,
        // even past the end of the turn.
        leases.acquire(Lease::new("R6", a.clone(), Kind::Shell, "adb install", Some(SHELL_RUNNING)), &mut pruned).unwrap();
        leases.shell_command_finished(1, "R6", true);
        leases.shell_turn_ended(1);
        assert!(leases.holder_of("R6").unwrap().view().expires_in_s.unwrap() > SHELL_GRACE.as_secs());
    }

    #[test]
    fn a_run_started_under_a_claim_hands_the_device_back_to_it() {
        let leases = Leases::default();
        let a = Holder::Session { id: 1, name: "a".into() };
        let b = Holder::Session { id: 2, name: "b".into() };
        let mut pruned = Vec::new();
        assert!(leases.acquire(Lease::new("R5", a.clone(), Kind::Claim, "testing login", Some(Duration::from_secs(600))), &mut pruned).unwrap());
        assert!(leases.acquire(Lease::new("R5", a.clone(), Kind::Run(3), "device.run", None), &mut pruned).unwrap());
        assert_eq!(leases.list().len(), 1, "one device, one visible lease");
        assert_eq!(leases.holder_of("R5").unwrap().kind, Kind::Run(3));
        let gone = leases.release_run(3);
        assert_eq!(gone.len(), 1);
        assert_eq!(leases.holder_of("R5").unwrap().kind, Kind::Claim);
        assert_eq!(leases.check("R5", &b).unwrap_err().code, "device.busy");
    }

    /// A stopped run's install may still be under way in Gradle's daemon: the device stays its
    /// holder's for the grace period, then lapses; a claim the holder had is left as it was.
    #[test]
    fn a_stopped_run_keeps_the_device_for_a_grace_period() {
        let leases = Leases::default();
        let a = Holder::Session { id: 1, name: "a".into() };
        let b = Holder::Session { id: 2, name: "b".into() };
        let mut pruned = Vec::new();
        leases.acquire(Lease::new("R5", a.clone(), Kind::Run(4), "device.run debug", None), &mut pruned).unwrap();
        let (gone, shown) = leases.stop_run(4, SHELL_GRACE);
        assert_eq!(gone.len(), 1);
        assert_eq!(gone[0].kind, Kind::Run(4));
        let grace = shown.expect("the grace lease is what others see");
        assert_eq!((grace.kind, grace.holder.clone()), (Kind::Shell, a.clone()));
        assert!(grace.view().expires_in_s.unwrap() <= SHELL_GRACE.as_secs());
        assert_eq!(leases.check("R5", &b).unwrap_err().code, "device.busy");
        assert!(leases.check("R5", &a).is_ok(), "the holder itself may run again");
        assert!(leases.release_run(4).is_empty(), "the worker's own release no longer drops it");
        let (_, shown) = leases.stop_run(5, SHELL_GRACE);
        assert!(shown.is_none(), "no run 5, nothing changes");
        leases.acquire(Lease::new("R6", a.clone(), Kind::Claim, "testing", Some(Duration::from_secs(600))), &mut pruned).unwrap();
        leases.acquire(Lease::new("R6", a.clone(), Kind::Run(6), "device.run debug", None), &mut pruned).unwrap();
        let (_, shown) = leases.stop_run(6, Duration::ZERO);
        assert!(shown.is_none(), "the claim outranks the grace lease");
        assert_eq!(leases.holder_of("R6").unwrap().kind, Kind::Claim);
    }

    #[test]
    fn the_sweeper_hands_back_a_lease_as_it_lapses() {
        let leases = Leases::default();
        assert!(leases.wait_expired(Duration::from_secs(5)).is_none(), "nothing held, nothing to sweep");
        assert!(leases.start_sweeper());
        assert!(!leases.start_sweeper(), "one sweeper at a time");
        let mut pruned = Vec::new();
        leases.acquire(Lease::new("R5", Holder::User, Kind::Claim, "testing", Some(Duration::from_millis(50))), &mut pruned).unwrap();
        let started = Instant::now();
        let mut gone = Vec::new();
        while gone.is_empty() {
            gone = leases.wait_expired(Duration::from_secs(5)).unwrap();
            assert!(started.elapsed() < Duration::from_secs(2), "the lapse was never seen");
        }
        assert_eq!(gone[0].device, "R5");
        assert!(leases.wait_expired(Duration::from_secs(5)).is_none());
        assert!(leases.start_sweeper(), "an emptied map stands the sweeper down");
    }
}
