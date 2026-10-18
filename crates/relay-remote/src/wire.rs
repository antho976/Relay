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
    "app.backup.list", "app.backup.now", "app.resources.watch",
    // Arbiter on the phone: its reads, a proposal's answer, and the halt and restart keys.
    "arbiter.decision.list", "arbiter.halt", "arbiter.order.list", "arbiter.proposal.list",
    "arbiter.proposal.resolve", "arbiter.restart", "arbiter.series", "arbiter.settings.get",
    "arbiter.strategy.get", "arbiter.summary",
    "avd.boot", "avd.list",
    "bus.ping", "bus.subscribe", "bus.unsubscribe",
    "dashboard.get",
    "device.build", "device.list", "device.run", "device.run.list", "device.run.stop",
    "file.create", "file.delete", "file.read", "file.rename", "file.restore", "file.restore_head",
    "file.search", "file.tree", "file.write",
    "git.branch.clean_merged", "git.branch.create", "git.branch.delete", "git.branch.switch",
    "git.branches", "git.commit", "git.diff", "git.diff.file", "git.fetch", "git.log", "git.pr.list",
    "git.pr.open", "git.push", "git.show", "git.stage", "git.status", "git.suggest_message",
    "git.unstage",
    "github.connect", "github.repo.list", "github.status",
    "guardrail.config.get", "guardrail.config.set", "guardrail.confirm", "guardrail.hold.get",
    "guardrail.holds.list", "guardrail.reject",
    // Avex's training history, read only.
    "gym.cardio", "gym.lift.get", "gym.lifts", "gym.series", "gym.session.get", "gym.sessions",
    "gym.summary",
    "integration.discard", "integration.get", "integration.list", "integration.request",
    "mailbox.list", "mailbox.outbox", "mailbox.send",
    "module.changelog.draft", "module.complete", "module.create", "module.delete", "module.get",
    "module.list", "module.reopen", "module.restore", "module.update",
    // Tally's sync (antho976/Tally), and for the Threads space on the phone (apps/relay-android)
    // the ledger's reads and the Undo of an entry a thread added.
    "money.invest.delete", "money.invest.list", "money.invest.restore", "money.invest.summary",
    "money.lists", "money.series", "money.summary", "money.sync", "money.tx.delete",
    "money.tx.list", "money.tx.restore",
    "notes.append", "notes.create", "notes.delete", "notes.get", "notes.list", "notes.pin",
    "notes.restore", "notes.standing", "notes.update",
    "notify.ack", "notify.ack_all", "notify.list", "notify.settings.get", "notify.settings.set",
    "overlap.list",
    "plugin.enable", "plugin.get", "plugin.list",
    "project.add", "project.clone", "project.get", "project.list", "project.remove", "project.update",
    "provider.list", "provider.refresh", "provider.update",
    "session.attach", "session.brief", "session.clear_restorable", "session.close", "session.create",
    "session.detach", "session.discard_restorable", "session.get", "session.input", "session.list",
    "session.park", "session.peers", "session.resize", "session.restorable", "session.resume",
    "session.scrollback", "session.spawn", "session.transcript", "session.update", "session.wake",
    "settings.get", "settings.reset",
    "skill.create", "skill.delete", "skill.enable", "skill.install", "skill.list", "skill.update",
    "task.activity", "task.approve", "task.attach", "task.changelog.write", "task.children",
    "task.create", "task.delete", "task.detach", "task.dispatch", "task.get", "task.label.add",
    "task.label.list", "task.label.remove", "task.link_commit", "task.list", "task.move",
    "task.parent.set", "task.relate", "task.restore", "task.unrelate", "task.update",
    // The Threads space on the phone: the same conversations as the PC's.
    "thread.create", "thread.delete", "thread.get", "thread.list", "thread.rename", "thread.send",
    "thread.set", "thread.stop",
    "ui.page.switch",
    "usage.get",
    "workspace.create", "workspace.discover", "workspace.list", "workspace.remove", "workspace.update",
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
        // Audit rows carry stored payloads (held actions, prompts, an old Arbiter key), and no
        // screen of the app reads them.
        for op in ["settings.set", "app.quit", "guardrail.grant.revoke", "worktree.remove", "os.open_url", "no.such", "audit.list", "audit.undo"] {
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
    fn hello_accepts_either_shape() {
        let pair: Hello = serde_json::from_str(r#"{"v":1,"pair":"ABCD-EFGH","device_name":"Pixel"}"#).unwrap();
        assert_eq!(pair.pair.as_deref(), Some("ABCD-EFGH"));
        let proof: Hello = serde_json::from_str(r#"{"v":1,"device":"abc","proof":"00"}"#).unwrap();
        assert_eq!(proof.device.as_deref(), Some("abc"));
        assert!(proof.pair.is_none());
    }
}
