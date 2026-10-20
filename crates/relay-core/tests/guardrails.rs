//! Phase 4 adversarial suite: effective policy, durable decisions, confirmation,
//! authorization and the installed enforcement adapters. Policy calls go through the bus.

mod common;

use common::{call_as as call, committed_repo, engine_with_project, err as error, git, git_command, ok_as as ok};
use relay_bus::{Actor, ErrorKind, Request};
use relay_core::engine::{Door, Engine};
use relay_core::Instance;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

struct Fixture {
    _root: tempfile::TempDir,
    repo: PathBuf,
    engine: Arc<Engine>,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let ws = root.path().join("ws");
        let repo = ws.join("app");
        committed_repo(&repo, &[("README.md", "relay\n")]);
        let engine = engine_with_project(root.path(), &ws, &repo);
        Self { _root: root, repo, engine }
    }

    fn session(&self, role: &str, provider: &str) -> Value {
        ok(
            &self.engine,
            Actor::User,
            "session.create",
            json!({"project_id": 1, "provider": provider, "role": role}),
        )
    }
}

fn count(engine: &Engine, table: &str) -> i64 {
    engine
        .store
        .lock()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0))
        .unwrap()
}

#[test]
fn effective_config_merges_without_erasing_project_overrides() {
    let fixture = Fixture::new();
    let engine = &fixture.engine;
    let defaults = ok(engine, Actor::User, "guardrail.config.get", json!({}));
    assert_eq!(defaults["caps"], json!({"files": 40, "lines": 2000}));

    let project = ok(
        engine,
        Actor::User,
        "guardrail.config.set",
        json!({"project_id": 1, "patch": {"caps": {"files": 3, "lines": 77}}}),
    );
    assert_eq!(project["caps"], json!({"files": 3, "lines": 77}));

    ok(
        engine,
        Actor::User,
        "guardrail.config.set",
        json!({"patch": {"destructive_write": {"min_removed_lines": 12}}}),
    );
    let effective = ok(
        engine,
        Actor::User,
        "guardrail.config.get",
        json!({"project_id": 1}),
    );
    assert_eq!(effective["caps"], json!({"files": 3, "lines": 77}));
    assert_eq!(effective["destructive_write"]["min_removed_lines"], 12);

    let invalid = call(
        engine,
        Actor::User,
        "guardrail.config.set",
        json!({"project_id": 1, "patch": {"caps": {"lines": 0}}}),
    );
    assert_eq!(error(&invalid).code, "guardrail.config");
    let after = ok(
        engine,
        Actor::User,
        "guardrail.config.get",
        json!({"project_id": 1}),
    );
    assert_eq!(after["caps"], json!({"files": 3, "lines": 77}), "invalid patch rolled back");
}

#[test]
fn refusals_holds_confirmation_replay_and_expiry_are_durable() {
    let fixture = Fixture::new();
    let engine = &fixture.engine;
    ok(
        engine,
        Actor::User,
        "guardrail.config.set",
        json!({"project_id": 1, "patch": {"protected_paths": ["secret/**"]}}),
    );
    let session = fixture.session("builder", "codex");
    let name = session["name"].as_str().unwrap();
    let actor = Actor::agent(name);
    let worktree = PathBuf::from(session["worktree"].as_str().unwrap());

    let refused = call(
        engine,
        actor.clone(),
        "guardrail.gate",
        json!({"session": name, "kind": "write", "path": "secret/key", "new_text": "x"}),
    );
    assert_eq!(error(&refused).kind, ErrorKind::Refused);
    assert_eq!(error(&refused).code, "guardrail.protected_path");
    assert_eq!(count(engine, "holds"), 0);
    assert_eq!(count(engine, "notifications"), 1);
    let audit = ok(engine, Actor::User, "audit.list", json!({"op_prefix": "guardrail.gate"}));
    assert!(audit["rows"].as_array().unwrap().iter().any(|row| {
        row["actor"] == format!("agent:{name}")
            && row["kind"] == "refused"
            && row["code"] == "guardrail.protected_path"
    }));

    let user_bypass = call(
        engine,
        Actor::User,
        "guardrail.gate",
        json!({"session": name, "kind": "write", "path": "secret/key", "new_text": "x"}),
    );
    assert_eq!(error(&user_bypass).kind, ErrorKind::Held);
    assert_eq!(error(&user_bypass).code, "guardrail.user_bypass");
    let bypass_id = error(&user_bypass).confirm.as_ref().unwrap().payload["hold_id"].as_i64().unwrap();
    let inspected = ok(engine, Actor::User, "guardrail.hold.get", json!({"hold_id": bypass_id}));
    assert_eq!(inspected["hold"]["state"], "open");
    assert_eq!(inspected["request"]["op"], "guardrail.gate");
    assert_eq!(inspected["request"]["payload"]["path"], "secret/key");
    assert_eq!(inspected["request"]["payload"]["new_text"], "x");
    assert!(inspected["request"].get("token").is_none());
    assert_eq!(error(&call(engine, actor.clone(), "guardrail.hold.get", json!({"hold_id": bypass_id}))).code, "actor.allowlist",
        "an agent must not inspect another actor's frozen action");
    // Nor read it back out of the audit trail, which stores the same payload.
    let held_row = ok(engine, Actor::User, "audit.list", json!({"op_prefix": "guardrail.gate"}))["rows"]
        .as_array().unwrap().iter()
        .find(|row| row["code"] == "guardrail.user_bypass").unwrap()["id"].clone();
    assert_eq!(ok(engine, Actor::User, "audit.get", json!({"audit_id": held_row}))["payload"]["new_text"], "x");
    for (op, payload) in [("audit.list", json!({"op_prefix": "guardrail.gate"})), ("audit.get", json!({"audit_id": held_row}))] {
        let refused = call(engine, actor.clone(), op, payload);
        assert_eq!(error(&refused).kind, ErrorKind::Refused, "{op}");
    }
    let bypass = ok(
        engine,
        Actor::User,
        "guardrail.confirm",
        json!({"hold_id": bypass_id}),
    );
    assert_eq!(bypass["hold"]["state"], "confirmed");
    assert_eq!(bypass["outcome"]["ok"], true);

    let old = (0..100).map(|line| format!("line-{line}\n")).collect::<String>();
    let next = (0..10).map(|line| format!("line-{line}\n")).collect::<String>();
    std::fs::write(worktree.join("large.txt"), old).unwrap();

    let before_holds = count(engine, "holds");
    let before_notifications = count(engine, "notifications");
    // A diff is measured against the file on disk, not against itself. Three lines out of a
    // hundred is not a destructive write, and used to be held only because the missing old
    // size was inferred from the diff and therefore always came out at 100%.
    let small = ok(
        engine,
        actor.clone(),
        "guardrail.check",
        json!({"project_id": 1, "kind": "write", "path": "large.txt", "diff": "-a\n-b\n-c\n"}),
    );
    assert_eq!(small["verdict"], "allow", "a three-line diff is not a destructive write");

    let wholesale: String = (0..60).map(|_| "-gone\n").collect();
    let check = ok(
        engine,
        actor.clone(),
        "guardrail.check",
        json!({"project_id": 1, "kind": "write", "path": "large.txt", "diff": wholesale}),
    );
    assert_eq!(check["verdict"], "hold");
    assert_eq!(count(engine, "holds"), before_holds, "dry run created a hold");
    assert_eq!(count(engine, "notifications"), before_notifications, "dry run notified");

    let held = call(
        engine,
        actor.clone(),
        "guardrail.gate",
        json!({"session": name, "kind": "write", "path": "large.txt", "new_text": next}),
    );
    assert_eq!(error(&held).kind, ErrorKind::Held);
    assert_eq!(error(&held).code, "guardrail.destructive_write");
    let hold_id = error(&held).confirm.as_ref().unwrap().payload["hold_id"].as_i64().unwrap();
    assert_eq!(count(engine, "holds"), before_holds + 1);
    assert_eq!(count(engine, "notifications"), before_notifications + 1);

    let confirm_id = Uuid::new_v4();
    let confirm = Request::new(Actor::User, "guardrail.confirm", json!({"hold_id": hold_id}))
        .with_id(confirm_id);
    let first = engine.dispatch(confirm.clone(), Door::InProcess);
    assert!(first.ok, "{:?}", first.error);
    let result = first.result.as_ref().unwrap();
    assert_eq!(result["hold"]["state"], "confirmed");
    assert_eq!(result["outcome"]["ok"], true);
    let replay = engine.dispatch(confirm, Door::InProcess);
    assert_eq!(replay.replayed, Some(true));
    assert_eq!(replay.result, first.result);

    let inbox = call(
        engine,
        actor.clone(),
        "mailbox.list",
        json!({"unread_only": true}),
    );
    assert_eq!(inbox.mail.as_ref().map(|mail| mail.priority), Some(2));
    let messages = inbox.result.as_ref().unwrap()["messages"].as_array().unwrap();
    assert!(messages.iter().all(|message| message["priority"] == true));
    for message in messages {
        ok(
            engine,
            actor.clone(),
            "mailbox.ack",
            json!({"message_id": message["id"]}),
        );
    }
    let after_ack = call(
        engine,
        actor.clone(),
        "session.get",
        json!({"session": name}),
    );
    assert!(after_ack.mail.is_none());

    let audit = ok(engine, Actor::User, "audit.list", json!({"op_prefix": "guardrail.gate"}));
    let confirmed = audit["rows"].as_array().unwrap().iter()
        .find(|row| row["req_id"] == confirm_id.to_string()).unwrap();
    assert_eq!(confirmed["actor"], "user");
    assert_eq!(confirmed["on_behalf_of"], format!("agent:{name}"));
    assert_eq!(confirmed["kind"], "ok");

    // The hook that asked has already failed the write, so the agent retries it. The person's
    // confirmation lets exactly that retry through, once.
    let retried = call(
        engine,
        actor.clone(),
        "guardrail.gate",
        json!({"session": name, "kind": "write", "path": "large.txt", "new_text": next}),
    );
    assert!(retried.ok, "the confirmed write was held again: {:?}", retried.error);
    assert_eq!(retried.result.as_ref().unwrap()["verdict"], "allow");
    let once_more = call(
        engine,
        actor.clone(),
        "guardrail.gate",
        json!({"session": name, "kind": "write", "path": "large.txt", "new_text": next}),
    );
    assert_eq!(error(&once_more).kind, ErrorKind::Held, "a confirmation covers one retry, not every one");
    // The held agent is told it may ask for the path instead of waiting.
    assert!(error(&once_more).hint.as_deref().is_some_and(|hint| hint.contains("guardrail.request")));

    let held_again = call(
        engine,
        actor,
        "guardrail.gate",
        json!({"session": name, "kind": "write", "path": "large.txt", "new_text": "tiny\n"}),
    );
    let expiring_id = error(&held_again).confirm.as_ref().unwrap().payload["hold_id"].as_i64().unwrap();
    // The checkout holds the test's uncommitted `large.txt` (RA-405).
    ok(engine, Actor::User, "session.close", json!({"session": name, "discard_changes": true}));
    let holds = ok(engine, Actor::User, "guardrail.holds.list", json!({"open_only": false}));
    let expired = holds["holds"].as_array().unwrap().iter()
        .find(|hold| hold["id"] == expiring_id).unwrap();
    assert_eq!(expired["state"], "expired");
    assert_eq!(expired["resolved_by"], "system");
}

#[test]
fn roles_scope_shape_exec_and_commit_caps_resist_bypass() {
    let fixture = Fixture::new();
    let engine = &fixture.engine;
    ok(
        engine,
        Actor::User,
        "guardrail.config.set",
        json!({"project_id": 1, "patch": {
            "caps": {"files": 1, "lines": 2},
            "shape_gates": [{"path": "manifest.json", "validator": "json_non_empty_object"}]
        }}),
    );
    let builder = fixture.session("builder", "codex");
    let reviewer = fixture.session("reviewer", "codex");
    let docs = fixture.session("docs", "codex");
    let builder_name = builder["name"].as_str().unwrap();
    let reviewer_name = reviewer["name"].as_str().unwrap();
    let docs_name = docs["name"].as_str().unwrap();

    let denied = call(
        engine,
        Actor::agent(reviewer_name),
        "guardrail.gate",
        json!({"session": reviewer_name, "kind": "exec", "command": "cargo test"}),
    );
    assert_eq!(error(&denied).code, "actor.allowlist");
    // RA-014: a Codex reviewer reaches the bus only through `$RELAY_BIN q …` in its shell, so
    // a single bus call, and a read-only look at the checkout, pass its closed shell. Anything
    // chained, redirected, substituted or acting as someone else does not.
    let gate = |command: &str| {
        call(
            engine,
            Actor::agent(reviewer_name),
            "guardrail.gate",
            json!({"session": reviewer_name, "kind": "exec", "command": command}),
        )
    };
    for command in [
        "$RELAY_BIN q session.bootstrap",
        "\"$RELAY_BIN\" q mailbox.send '{\"to\":\"x\",\"text\":\"fix line 3; then rerun\"}'",
        "$RELAY_BIN q session.done '{\"summary\":\"ok\"}'",
        "bash -lc '$RELAY_BIN q session.bootstrap'",
        "git diff main...HEAD", "git -C sub log --oneline -5", "rg -n 'fn main' src", "cat src/lib.rs",
    ] {
        assert!(gate(command).ok, "a reviewer may run {command:?}: {:?}", gate(command).error);
    }
    for command in [
        "$RELAY_BIN q session.bootstrap; touch x", "$RELAY_BIN q session.bootstrap > out.json",
        "$RELAY_BIN --actor user q guardrail.confirm '{\"hold_id\":1}'", "RELAY_SESSION= $RELAY_BIN q task.create",
        "$RELAY_BIN q x \"$(touch y)\"", "relay-evil q session.done", "git -c core.pager=sh diff",
        "git diff --output=patch.txt", "rg --pre ./run.sh x", "git commit -m x", "cat a | tee b",
    ] {
        assert_eq!(error(&gate(command)).code, "actor.allowlist", "a reviewer may not run {command:?}");
    }
    let query = call(
        engine,
        Actor::agent(reviewer_name),
        "guardrail.check",
        json!({"project_id": 1, "kind": "exec", "command": "cargo test"}),
    );
    assert!(query.ok, "all roles retain all queries");
    let future_write = call(
        engine,
        Actor::agent(reviewer_name),
        "file.write",
        json!({"project_id": 1, "path": "review.txt", "text": "no"}),
    );
    assert_eq!(
        error(&future_write).code,
        "actor.allowlist",
        "authorization runs before implementation availability"
    );

    let cross_session = call(
        engine,
        Actor::agent(builder_name),
        "guardrail.gate",
        json!({"session": docs_name, "kind": "exec", "command": "cargo test"}),
    );
    assert_eq!(error(&cross_session).code, "actor.scope");
    let docs_allowed = call(
        engine,
        Actor::agent(docs_name),
        "guardrail.gate",
        json!({"session": docs_name, "kind": "exec", "command": "cargo test"}),
    );
    assert!(docs_allowed.ok, "docs baseline includes guardrail.gate");

    let denied_command = call(
        engine,
        Actor::agent(builder_name),
        "guardrail.gate",
        json!({"session": builder_name, "kind": "exec", "command": "git reset --hard HEAD"}),
    );
    assert_eq!(error(&denied_command).code, "guardrail.command");
    assert_eq!(error(&denied_command).kind, ErrorKind::Refused);
    // The agent works around a refusal; the person is not pinged for it, though it is filed.
    let refusals = |unread_only: bool| ok(engine, Actor::User, "notify.list", json!({"category": "guardrail", "unread_only": unread_only}))["notifications"]
        .as_array().unwrap().iter().filter(|card| card["title"] == "Guardrail refusal").count();
    assert_eq!((refusals(false), refusals(true)), (1, 0));

    let shape = call(
        engine,
        Actor::agent(builder_name),
        "guardrail.gate",
        json!({"session": builder_name, "kind": "write", "path": "manifest.json", "new_text": "{}"}),
    );
    assert_eq!(error(&shape).code, "guardrail.shape_gate");
    assert_eq!(error(&shape).kind, ErrorKind::Held);
    let shape_id = error(&shape).confirm.as_ref().unwrap().payload["hold_id"].as_i64().unwrap();
    let rejected = ok(
        engine,
        Actor::User,
        "guardrail.reject",
        json!({"hold_id": shape_id, "reason": "manifest cannot be empty"}),
    );
    assert_eq!(rejected["hold"]["state"], "rejected");
    let duplicate_reject = call(
        engine,
        Actor::User,
        "guardrail.reject",
        json!({"hold_id": shape_id}),
    );
    assert_eq!(error(&duplicate_reject).code, "guardrail.hold_resolved");

    let worktree = PathBuf::from(builder["worktree"].as_str().unwrap());
    std::fs::write(worktree.join("one.txt"), "one\ntwo\n").unwrap();
    std::fs::write(worktree.join("two.txt"), "three\nfour\n").unwrap();
    git(&worktree, &["add", "one.txt", "two.txt"]);
    let capped = call(
        engine,
        Actor::agent(builder_name),
        "guardrail.gate",
        json!({"session": builder_name, "kind": "commit"}),
    );
    assert_eq!(error(&capped).code, "guardrail.cap");
    assert_eq!(error(&capped).kind, ErrorKind::Refused);
}

#[test]
fn sessions_install_git_and_claude_hooks_without_clobbering_local_settings() {
    let fixture = Fixture::new();
    let session = fixture.session("builder", "claude");
    let name = session["name"].as_str().unwrap();
    let worktree = PathBuf::from(session["worktree"].as_str().unwrap());
    let hook_dir = git_command(&worktree)
        .args(["config", "--worktree", "--get", "core.hooksPath"])
        .output()
        .unwrap();
    assert!(hook_dir.status.success());
    let hook = PathBuf::from(String::from_utf8(hook_dir.stdout).unwrap().trim()).join("pre-commit");
    let metadata = std::fs::metadata(&hook).unwrap();
    use std::os::unix::fs::PermissionsExt;
    assert_ne!(metadata.permissions().mode() & 0o111, 0);
    let script = std::fs::read_to_string(&hook).unwrap();
    assert!(script.contains("guardrail.gate"));
    assert!(script.contains(name));
    assert!(script.contains("printf '{\"session\":\"%s\",\"kind\":\"commit\"}' \"$RELAY_SESSION\""));

    let claude_dir = worktree.join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(
        claude_dir.join("settings.local.json"),
        r#"{"permissions":{"allow":["Bash(cargo test)"]},"hooks":{"PreToolUse":[{"matcher":"Read","hooks":[{"type":"command","command":"true"}]}]}}"#,
    ).unwrap();
    ok(
        &fixture.engine,
        Actor::User,
        "settings.set",
        json!({"path": "providers.claude.path", "value": "/bin/sh"}),
    );
    ok(
        &fixture.engine,
        Actor::User,
        "session.spawn",
        json!({"session": name}),
    );
    // session.spawn itself installs the guardrail and reporting hooks, beside the user's own.
    let installed = || {
        let settings: Value = serde_json::from_str(
            &std::fs::read_to_string(claude_dir.join("settings.local.json")).unwrap(),
        ).unwrap();
        assert_eq!(settings["permissions"]["allow"][0], "Bash(cargo test)");
        let groups = settings["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(groups.len(), 2);
        assert!(groups.iter().any(|group| group["matcher"] == "Edit|Write|MultiEdit|Bash"));
        for event in ["SessionStart", "PostToolUse", "Stop", "Notification"] {
            let groups = settings["hooks"][event].as_array().unwrap();
            assert_eq!(groups.len(), 1, "Relay installed {event} reporting");
            assert!(groups[0]["hooks"][0]["command"].as_str().unwrap().contains("hook claude-report"));
        }
    };
    installed();
    // Installing again (the next launch) replaces Relay's groups rather than adding to them.
    relay_core::hooks::install_claude(&worktree, Instance::Test, Path::new("relay")).unwrap();
    installed();
    let mcp: Value = serde_json::from_str(&std::fs::read_to_string(worktree.join(".relay/relay.mcp.json")).unwrap()).unwrap();
    assert_eq!(mcp["mcpServers"]["relay"]["args"], json!(["--instance","test","mcp"]));

    let exclude = std::fs::read_to_string(fixture.repo.join(".git/info/exclude")).unwrap();
    assert!(exclude.lines().any(|line| line == ".claude/settings.local.json"));

    ok(
        &fixture.engine,
        Actor::User,
        "session.close",
        json!({"session": name, "remove_worktree": false}),
    );
    let settings: Value = serde_json::from_str(
        &std::fs::read_to_string(claude_dir.join("settings.local.json")).unwrap(),
    ).unwrap();
    let groups = settings["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(groups.len(), 1, "close removed only Relay's Claude hook");
    assert_eq!(groups[0]["matcher"], "Read");
    for event in ["SessionStart", "PostToolUse", "Stop", "Notification"] {
        assert!(settings["hooks"][event].as_array().unwrap().is_empty(), "close removed Relay's {event} hook");
    }
    assert!(!worktree.join(".relay/relay.mcp.json").exists(), "close removed Relay's MCP config");

    let primary = ok(
        &fixture.engine,
        Actor::User,
        "session.create",
        json!({"project_id": 1, "provider": "codex", "worktree": "primary"}),
    );
    let primary_name = primary["name"].as_str().unwrap();
    let configured = git_command(&fixture.repo)
        .args(["config", "--worktree", "--get", "core.hooksPath"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&configured.stdout).contains(".relay/hooks"));
    ok(
        &fixture.engine,
        Actor::User,
        "session.close",
        json!({"session": primary_name}),
    );
    let restored = git_command(&fixture.repo)
        .args(["config", "--worktree", "--get", "core.hooksPath"])
        .output()
        .unwrap();
    // The primary checkout had no hook path of its own, so it has none again (RA-106).
    assert!(!restored.status.success(), "close pinned {}", String::from_utf8_lossy(&restored.stdout));
}

#[test]
fn codex_hooks_add_guardrails_and_lifecycle_without_clobbering_project_hooks() {
    let root = tempfile::tempdir().unwrap();
    let codex_dir = root.path().join(".codex");
    std::fs::create_dir_all(&codex_dir).unwrap();
    std::fs::write(
        codex_dir.join("hooks.json"),
        r#"{"description":"mine","hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"true"}]}]}}"#,
    ).unwrap();

    relay_core::hooks::install_codex(root.path(), Instance::Test, Path::new("/opt/relay")).unwrap();
    let installed: Value = serde_json::from_str(&std::fs::read_to_string(codex_dir.join("hooks.json")).unwrap()).unwrap();
    assert_eq!(installed["description"], "mine");
    let pre = installed["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(pre.len(), 2);
    assert!(pre.iter().any(|group| group["hooks"][0]["command"].as_str().unwrap().contains("hook codex-pre-tool")));
    for event in ["SessionStart", "PostToolUse", "Stop"] {
        assert!(installed["hooks"][event][0]["hooks"][0]["command"].as_str().unwrap().contains("hook codex-report"));
    }

    relay_core::hooks::uninstall_codex(root.path()).unwrap();
    let removed: Value = serde_json::from_str(&std::fs::read_to_string(codex_dir.join("hooks.json")).unwrap()).unwrap();
    assert_eq!(removed["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);
    assert_eq!(removed["hooks"]["PreToolUse"][0]["hooks"][0]["command"], "true");
    for event in ["SessionStart", "PostToolUse", "Stop"] {
        assert!(removed["hooks"][event].as_array().unwrap().is_empty());
    }
}

fn events(rx: &mut tokio::sync::broadcast::Receiver<relay_bus::Event>) -> Vec<relay_bus::Event> {
    let mut out = Vec::new();
    while let Ok(event) = rx.try_recv() {
        out.push(event);
    }
    out
}

#[test]
fn workspace_layer_sits_between_global_and_project_and_null_clears_back() {
    let fixture = Fixture::new();
    let engine = &fixture.engine;
    let set = |payload: Value| ok(engine, Actor::User, "guardrail.config.set", payload);
    let get = |payload: Value| ok(engine, Actor::User, "guardrail.config.get", payload);
    set(json!({"patch": {"caps": {"files": 30}}}));
    set(json!({"workspace_id": 1, "patch": {"caps": {"files": 20}, "denied_commands": ["make deploy"]}}));
    set(json!({"project_id": 1, "patch": {"caps": {"files": 10}}}));

    assert_eq!(get(json!({}))["caps"], json!({"files": 30, "lines": 2000}));
    assert_eq!(get(json!({"workspace_id": 1}))["caps"]["files"], 20);
    let project = get(json!({"project_id": 1}));
    assert_eq!(project["caps"], json!({"files": 10, "lines": 2000}));
    assert_eq!(project["denied_commands"], json!(["make deploy"]), "the workspace layer reaches its projects");

    let layers = ok(engine, Actor::User, "guardrail.config.layers", json!({"project_id": 1}));
    assert_eq!(layers["scope"], "project");
    assert_eq!(layers["workspace_id"], 1);
    assert_eq!(layers["inherited"]["caps"]["files"], 20);
    assert_eq!(layers["overrides"], json!({"caps": {"files": 10}}));
    assert_eq!(layers["sources"]["caps.files"], "project");
    assert_eq!(layers["sources"]["caps.lines"], "default");
    assert_eq!(layers["sources"]["denied_commands"], "workspace");
    let global = ok(engine, Actor::User, "guardrail.config.layers", json!({}));
    assert_eq!(global["scope"], "global");
    assert_eq!(global["sources"]["caps.files"], "global");
    assert_eq!(global["inherited"]["caps"]["files"], 40, "global inherits the defaults");

    // Clearing the project's only override leaves an empty `caps` object behind. That used to
    // replace the whole `caps` subtree and make every later read fail to deserialize.
    let cleared = set(json!({"project_id": 1, "patch": {"caps": {"files": null}}}));
    assert_eq!(cleared["caps"], json!({"files": 20, "lines": 2000}));
    let layers = ok(engine, Actor::User, "guardrail.config.layers", json!({"project_id": 1}));
    assert_eq!(layers["overrides"], json!({}));
    assert_eq!(layers["sources"]["caps.files"], "workspace");
    set(json!({"workspace_id": 1, "patch": {"caps": null}}));
    assert_eq!(get(json!({"project_id": 1}))["caps"]["files"], 30);
    set(json!({"patch": {"caps": {"files": null}}}));
    assert_eq!(get(json!({"project_id": 1}))["caps"]["files"], 40, "every layer cleared: the default");

    // A stray empty object written straight into settings merges instead of wiping defaults.
    ok(engine, Actor::User, "settings.set", json!({"path": "guardrails.projects.1.caps", "value": {}}));
    assert_eq!(get(json!({"project_id": 1}))["caps"], json!({"files": 40, "lines": 2000}));

    let both = call(engine, Actor::User, "guardrail.config.get", json!({"workspace_id": 1, "project_id": 1}));
    assert_eq!(error(&both).code, "guardrail.scope");
    let missing = call(engine, Actor::User, "guardrail.config.set", json!({"workspace_id": 9, "patch": {}}));
    assert_eq!(error(&missing).code, "workspace.not_found");
    let nested = call(engine, Actor::User, "guardrail.config.set", json!({"patch": {"workspaces": {"1": {}}}}));
    assert_eq!(error(&nested).code, "guardrail.config");
}

#[test]
fn a_bad_guardrails_key_is_refused_on_write_and_ignored_when_already_stored() {
    let fixture = Fixture::new();
    let engine = &fixture.engine;
    let rows = count(engine, "settings");
    // settings.set reaches the tree guardrail.config.set writes, so it is checked the same way.
    for (path, value) in [
        ("guardrails.caps.file", json!(3)),
        ("guardrails.caps.files", json!("40")),
        ("guardrails.caps", json!({"files": 0})),
        ("guardrails.projects.1.destructive_write.min_removed", json!(5)),
        ("guardrails", json!({"roles": {"reviewr": []}})),
        ("", json!({"guardrails": {"shape_gates": [{"path": "a.json", "validator": "json", "strict": true}]}})),
    ] {
        let refused = call(engine, Actor::User, "settings.set", json!({"path": path, "value": value}));
        assert_eq!(error(&refused).code, "guardrail.config", "{path} {value}");
    }
    let typo = call(engine, Actor::User, "guardrail.config.set", json!({"patch": {"caps": {"file": 3}}}));
    assert_eq!(error(&typo).code, "guardrail.config", "the typed write path stays strict");
    assert_eq!(count(engine, "settings"), rows, "every refused write rolled back");
    ok(engine, Actor::User, "settings.set", json!({"path": "guardrails.caps.files", "value": 12}));
    assert_eq!(ok(engine, Actor::User, "guardrail.config.get", json!({}))["caps"]["files"], 12);

    // A store that already holds keys this build does not know — a newer build's, or one
    // written before the check — still yields a working config, at every layer.
    for (path, value) in [
        ("guardrails.caps.file", "3"),
        ("guardrails.future_rule", r#"{"on":true}"#),
        ("guardrails.workspaces.1.destructive_write.newer_knob", "1"),
        ("guardrails.projects.1.shape_gates", r#"[{"path":"a.json","validator":"json","strict":true}]"#),
    ] {
        engine.store.lock()
            .execute("INSERT INTO settings(path, value, updated_at) VALUES (?1, ?2, '2026-01-01T00:00:00Z')", [path, value])
            .unwrap();
    }
    assert_eq!(ok(engine, Actor::User, "guardrail.config.get", json!({}))["caps"], json!({"files": 12, "lines": 2000}));
    let project = ok(engine, Actor::User, "guardrail.config.get", json!({"project_id": 1}));
    assert_eq!(project["shape_gates"], json!([{"path": "a.json", "validator": "json"}]));
    ok(engine, Actor::User, "guardrail.config.layers", json!({"project_id": 1}));
    let session = fixture.session("builder", "codex");
    let name = session["name"].as_str().unwrap();
    let agent = Actor::agent(name);
    ok(engine, agent.clone(), "bus.whoami", json!({}));
    ok(engine, agent.clone(), "guardrail.gate", json!({"session": name, "kind": "write", "path": "src/a.rs", "new_text": "x"}));
    ok(engine, agent, "session.intent", json!({"session": name, "text": "still works"}));
    // The stray rows clear the ordinary way.
    ok(engine, Actor::User, "settings.reset", json!({"path": "guardrails.future_rule"}));
    ok(engine, Actor::User, "guardrail.config.set", json!({"patch": {"caps": {"file": null}}}));
    assert_eq!(ok(engine, Actor::User, "guardrail.config.get", json!({}))["caps"]["files"], 12);
}

/// RA-318 / RA-319: a scalar where a layer belongs, and a write root that is not absolute, are
/// refused on write; a scalar layer already stored fails the read closed instead of panicking.
#[test]
fn a_scalar_layer_or_relative_write_root_is_refused_and_never_panics() {
    let fixture = Fixture::new();
    let engine = &fixture.engine;
    for path in ["guardrails.projects.1", "guardrails.workspaces.1"] {
        let refused = call(engine, Actor::User, "settings.set", json!({"path": path, "value": false}));
        assert_eq!(error(&refused).code, "guardrail.config", "{path}");
    }
    for root in ["~/scratch", "build/out"] {
        let refused = call(engine, Actor::User, "guardrail.config.set", json!({"patch": {"allowed_write_roots": [root]}}));
        assert_eq!(error(&refused).code, "guardrail.config", "{root}");
    }
    ok(engine, Actor::User, "guardrail.config.set", json!({"patch": {"allowed_write_roots": ["/srv/scratch"]}}));

    engine.store.lock()
        .execute("INSERT INTO settings(path, value, updated_at) VALUES ('guardrails.projects.1', 'false', '2026-01-01T00:00:00Z')", [])
        .unwrap();
    let read = call(engine, Actor::User, "guardrail.config.get", json!({"project_id": 1}));
    assert_eq!(error(&read).code, "guardrail.config", "a scalar layer fails the read, closed");
    ok(engine, Actor::User, "guardrail.config.get", json!({}));
    ok(engine, Actor::User, "settings.reset", json!({"path": "guardrails.projects.1"}));
    ok(engine, Actor::User, "guardrail.config.get", json!({"project_id": 1}));
}

/// RA-337: an agent's hold and request listings stop at its own project; the user's do not.
#[test]
fn an_agent_lists_only_its_own_projects_holds_and_requests() {
    let fixture = Fixture::new();
    let engine = &fixture.engine;
    let session = fixture.session("builder", "claude");
    let agent = Actor::agent(session["name"].as_str().unwrap());
    for op in ["guardrail.holds.list", "guardrail.requests.list"] {
        ok(engine, agent.clone(), op, json!({}));
        ok(engine, agent.clone(), op, json!({"project_id": 1}));
        assert_eq!(error(&call(engine, agent.clone(), op, json!({"project_id": 2}))).code, "actor.scope", "{op}");
        ok(engine, Actor::User, op, json!({"project_id": 2}));
    }
}

#[test]
fn an_agent_can_ask_but_only_the_user_answers_and_once_means_once() {
    let fixture = Fixture::new();
    let engine = &fixture.engine;
    let session = fixture.session("builder", "claude");
    let name = session["name"].as_str().unwrap();
    let agent = Actor::agent(name);
    let mut rx = engine.subscribe();
    let push = json!({"session": name, "kind": "exec", "command": "git push --force origin main"});

    let refused = call(engine, agent.clone(), "guardrail.gate", push.clone());
    let refusal = error(&refused);
    assert_eq!(refusal.code, "guardrail.command");
    assert!(refusal.hint.as_deref().unwrap().contains("guardrail.request"), "the agent is told it can ask");
    assert_eq!(refusal.details.as_ref().unwrap()["exception"],
        json!({"kind": "command", "value": "git push --force origin main"}));

    let asked = ok(engine, agent.clone(), "guardrail.request", json!({
        "kind": "command", "value": "git push --force origin main",
        "reason": "the remote branch was rebased by the user and must be replaced",
    }));
    assert_eq!(asked["created"], true);
    assert_eq!(asked["wait_for"], "guardrail.request_resolved");
    assert_eq!(asked["request"]["state"], "open");
    assert_eq!(asked["request"]["session"], name, "the engine fills the bound session");
    let id = asked["request"]["id"].as_i64().unwrap();
    let again = ok(engine, agent.clone(), "guardrail.request", json!({
        "kind": "command", "value": "git push --force origin main", "reason": "still need it",
    }));
    assert_eq!(again["created"], false);
    assert_eq!(again["request"]["id"], id, "asking twice is one question");

    let holds = ok(engine, Actor::User, "guardrail.holds.list", json!({}));
    let hold = holds["holds"].as_array().unwrap().iter().find(|hold| hold["id"] == id).unwrap();
    assert_eq!(hold["op"], "guardrail.request");
    assert_eq!(hold["policy"], "exception");
    let notices = ok(engine, Actor::User, "notify.list", json!({}));
    assert!(notices.to_string().contains("Agent requests a guardrail exception"), "{notices}");
    let seen = events(&mut rx);
    assert!(seen.iter().any(|event| event.ev == "guardrail.requested" && event.payload["request_id"] == id));
    assert!(seen.iter().any(|event| event.ev == "guardrail.held" && event.payload["policy"] == "exception"));

    // No self-approval: the answer is user-only, on the bus and through a shell.
    let own = call(engine, agent.clone(), "guardrail.confirm", json!({"hold_id": id}));
    assert_eq!(error(&own).code, "actor.allowlist");
    let shell = call(engine, agent.clone(), "guardrail.gate", json!({
        "session": name, "kind": "exec",
        "command": format!("$RELAY_BIN --actor user q guardrail.confirm '{{\"hold_id\":{id}}}'"),
    }));
    assert_eq!(error(&shell).code, "guardrail.self_approval");

    let approved = ok(engine, Actor::User, "guardrail.confirm", json!({"hold_id": id, "scope": "once"}));
    assert_eq!(approved["hold"]["state"], "confirmed");
    assert_eq!(approved["outcome"]["result"]["active"], true);
    let seen = events(&mut rx);
    let resolved = seen.iter().find(|event| event.ev == "guardrail.request_resolved").unwrap();
    assert_eq!(resolved.payload["request_id"], id);
    assert_eq!(resolved.payload["state"], "confirmed");
    assert_eq!(resolved.payload["scope"], "once");
    let inbox = ok(engine, agent.clone(), "mailbox.list", json!({"unread_only": true}));
    assert!(inbox.to_string().contains("approved for one use"), "{inbox}");

    // A grant is for the exact command: a longer one is not covered.
    let wider = call(engine, agent.clone(), "guardrail.gate",
        json!({"session": name, "kind": "exec", "command": "git push --force origin main other"}));
    assert_eq!(error(&wider).code, "guardrail.command");
    // Every denied command in a line needs its own grant.
    let chained = call(engine, agent.clone(), "guardrail.gate",
        json!({"session": name, "kind": "exec", "command": "git reset --hard && git push --force origin main"}));
    assert_eq!(error(&chained).code, "guardrail.command");
    assert!(error(&chained).message.contains("git reset --hard"), "{}", error(&chained).message);
    assert!(ok(engine, agent.clone(), "guardrail.request.get", json!({"request_id": id}))["active"] == true,
        "a refused action does not use the grant up");

    ok(engine, agent.clone(), "guardrail.gate",
        json!({"session": name, "kind": "exec", "command": "cd repo && git push --force origin main"}));
    let seen = events(&mut rx);
    assert!(seen.iter().any(|event| event.ev == "guardrail.grant_used" && event.payload["request_id"] == id));
    assert!(seen.iter().any(|event| event.ev == "guardrail.resolved" && event.payload["state"] == "used"),
        "a spent one-use grant is announced so every surface drops it");
    let used = ok(engine, agent.clone(), "guardrail.request.get", json!({"request_id": id}));
    assert_eq!(used["active"], false);
    assert_eq!(used["uses"], 1);
    assert!(used["used_at"].is_string());
    let second = call(engine, agent.clone(), "guardrail.gate", push);
    assert_eq!(error(&second).code, "guardrail.command", "once means once");
    let active = ok(engine, Actor::User, "guardrail.requests.list", json!({"state": "active"}));
    assert_eq!(active["requests"], json!([]));
}

#[test]
fn session_grants_cover_paths_roots_and_caps_until_revoked_and_denials_say_why() {
    let fixture = Fixture::new();
    let engine = &fixture.engine;
    ok(engine, Actor::User, "guardrail.config.set", json!({"project_id": 1, "patch": {
        "protected_paths": ["secret/**"], "caps": {"files": 1, "lines": 2},
    }}));
    let session = fixture.session("builder", "codex");
    let name = session["name"].as_str().unwrap();
    let agent = Actor::agent(name);
    let write = |path: &str| call(engine, agent.clone(), "guardrail.gate",
        json!({"session": name, "kind": "write", "path": path, "new_text": "x\n"}));
    let ask = |kind: &str, value: &str, scope: &str| {
        ok(engine, agent.clone(), "guardrail.request", json!({
            "kind": kind, "value": value, "reason": "needed for the task", "scope": scope,
        }))["request"]["id"].as_i64().unwrap()
    };

    assert_eq!(error(&write("secret/key")).code, "guardrail.protected_path");
    let path_grant = ask("path", "secret/key", "session");
    let pending = ok(engine, Actor::User, "guardrail.requests.list", json!({"state": "open"}));
    assert_eq!(pending["requests"][0]["requested_scope"], "session");
    // Approved with no scope: what the agent asked for.
    ok(engine, Actor::User, "guardrail.confirm", json!({"hold_id": path_grant}));
    assert!(write("secret/key").ok);
    assert!(write("secret/key").ok, "a session grant lasts");
    assert_eq!(error(&write("secret/other")).code, "guardrail.protected_path", "only the granted path");
    let listed = ok(engine, Actor::User, "guardrail.requests.list", json!({"state": "active", "session": name}));
    assert_eq!(listed["requests"][0]["uses"], 2);
    ok(engine, Actor::User, "guardrail.grant.revoke", json!({"request_id": path_grant}));
    assert_eq!(error(&write("secret/key")).code, "guardrail.protected_path", "revoked");
    let twice = call(engine, Actor::User, "guardrail.grant.revoke", json!({"request_id": path_grant}));
    assert_eq!(error(&twice).code, "guardrail.grant_inactive");
    let agent_revoke = call(engine, agent.clone(), "guardrail.grant.revoke", json!({"request_id": path_grant}));
    assert_eq!(error(&agent_revoke).code, "actor.allowlist");

    // An absolute root outside every write root.
    let outside = "/opt/relay-grant-test/out.txt";
    let refused = write(outside);
    assert_eq!(error(&refused).code, "guardrail.write_root");
    assert_eq!(error(&refused).details.as_ref().unwrap()["exception"]["kind"], "path");
    let root = ask("path", "/opt/relay-grant-test", "once");
    ok(engine, Actor::User, "guardrail.confirm", json!({"hold_id": root}));
    assert!(write(outside).ok);
    assert_eq!(error(&write(outside)).code, "guardrail.write_root");

    // Commit caps.
    let worktree = PathBuf::from(session["worktree"].as_str().unwrap());
    std::fs::write(worktree.join("one.txt"), "one\ntwo\n").unwrap();
    std::fs::write(worktree.join("two.txt"), "three\nfour\n").unwrap();
    git(&worktree, &["add", "one.txt", "two.txt"]);
    let commit = || call(engine, agent.clone(), "guardrail.gate", json!({"session": name, "kind": "commit"}));
    let capped = commit();
    assert_eq!(error(&capped).code, "guardrail.cap");
    let suggested = error(&capped).details.as_ref().unwrap()["exception"]["value"].as_str().unwrap().to_string();
    assert_eq!(suggested, "files=2 lines=4");
    let small = ask("cap", "files=2 lines=3", "once");
    ok(engine, Actor::User, "guardrail.confirm", json!({"hold_id": small}));
    assert_eq!(error(&commit()).code, "guardrail.cap", "a grant below the commit's size does not cover it");
    let enough = ask("cap", &suggested, "once");
    ok(engine, Actor::User, "guardrail.confirm", json!({"hold_id": enough}));
    assert!(commit().ok);

    // A denial reaches the agent with its reason.
    let mut rx = engine.subscribe();
    let denied = ask("command", "rm -rf build", "once");
    let rejected = ok(engine, Actor::User, "guardrail.reject", json!({"hold_id": denied, "reason": "use cargo clean"}));
    assert_eq!(rejected["hold"]["state"], "rejected");
    let read = ok(engine, agent.clone(), "guardrail.request.get", json!({"request_id": denied}));
    assert_eq!(read["state"], "rejected");
    assert_eq!(read["denial_reason"], "use cargo clean");
    assert!(events(&mut rx).iter().any(|event| event.ev == "guardrail.request_resolved"
        && event.payload["state"] == "rejected" && event.payload["reason"] == "use cargo clean"));

    let bad = call(engine, agent.clone(), "guardrail.request", json!({"kind": "path", "value": "../up", "reason": "x"}));
    assert_eq!(error(&bad).code, "guardrail.request");
    let no_reason = call(engine, agent, "guardrail.request", json!({"kind": "command", "value": "x", "reason": " "}));
    assert_eq!(error(&no_reason).code, "guardrail.request");
}

#[test]
fn agents_cannot_answer_guardrails_as_the_user_from_a_shell() {
    let fixture = Fixture::new();
    let engine = &fixture.engine;
    let session = fixture.session("builder", "codex");
    let name = session["name"].as_str().unwrap();
    let agent = Actor::agent(name);
    let exec = |command: &str| call(engine, agent.clone(), "guardrail.gate",
        json!({"session": name, "kind": "exec", "command": command}));
    for command in [
        "relay q guardrail.confirm '{\"hold_id\":1}'",
        "/usr/local/bin/relay --actor user q task.list",
        "$RELAY_BIN --actor=user q settings.get",
        "env -u RELAY_SESSION relay q guardrail.reject '{\"hold_id\":1}'",
        "RELAY_SESSION= relay q task.list",
        "cargo test && relay q guardrail.config.set '{\"patch\":{}}'",
    ] {
        let response = exec(command);
        assert_eq!(error(&response).code, "guardrail.self_approval", "{command}");
    }
    for command in ["relay q task.list", "grep guardrail.confirm docs/engine/BUS.md", "relay q guardrail.check '{}'"] {
        assert!(exec(command).ok, "{command} is not self-approval");
    }

    // A command grant does not lift it.
    let sneaky = "relay q guardrail.confirm '{\"hold_id\":1}'";
    let id = ok(engine, agent.clone(), "guardrail.request", json!({"kind": "command", "value": sneaky, "reason": "x"}))
        ["request"]["id"].as_i64().unwrap();
    ok(engine, Actor::User, "guardrail.confirm", json!({"hold_id": id, "scope": "session"}));
    assert_eq!(error(&exec(sneaky)).code, "guardrail.self_approval");
}

#[test]
fn every_role_may_ask_for_an_exception_but_only_for_itself() {
    let fixture = Fixture::new();
    let engine = &fixture.engine;
    let reviewer = fixture.session("reviewer", "codex");
    let builder = fixture.session("builder", "codex");
    let reviewer_name = reviewer["name"].as_str().unwrap();
    let asked = call(engine, Actor::agent(reviewer_name), "guardrail.request",
        json!({"kind": "command", "value": "cargo publish --dry-run", "reason": "verify packaging"}));
    assert!(asked.ok, "{:?}", asked.error);
    let ops = ok(engine, Actor::agent(reviewer_name), "bus.ops", json!({}));
    let row = ops["ops"].as_array().unwrap().iter().find(|op| op["name"] == "guardrail.request").unwrap();
    assert_ne!(row["call"], "no", "{}", row["why"]);
    let foreign = call(engine, Actor::agent(reviewer_name), "guardrail.request", json!({
        "session": builder["name"], "kind": "command", "value": "x", "reason": "x",
    }));
    assert_eq!(error(&foreign).code, "actor.scope");
    let bootstrap = ok(engine, Actor::agent(reviewer_name), "session.bootstrap", json!({}));
    assert!(bootstrap["guardrails"]["exceptions"].as_str().unwrap().contains("guardrail.request"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_approval_that_lands_before_the_wait_still_wakes_it_and_explain_sees_it() {
    use relay_core::socket::{Client, SocketServer};
    let fixture = Fixture::new();
    let engine = &fixture.engine;
    let session = fixture.session("builder", "claude");
    let name = session["name"].as_str().unwrap();
    let agent = Actor::agent(name);
    let sdk = "/opt/relay-grant-test/Sdk";
    let plan = json!({"project_id": 1, "paths": [format!("{sdk}/cmdline-tools/latest/bin/sdkmanager")]});

    let before = ok(engine, agent.clone(), "guardrail.explain", plan.clone());
    assert_eq!(before["verdict"], "refuse");
    assert_eq!(before["paths"][0]["policy"], "guardrail.write_root");
    let id = ok(engine, agent.clone(), "guardrail.request", json!({
        "kind": "path", "value": sdk, "reason": "install the emulator tools", "scope": "session",
    }))["request"]["id"].as_i64().unwrap();
    // The person answers within a second, before the agent's bus.wait subscribes.
    ok(engine, Actor::User, "guardrail.confirm", json!({"hold_id": id}));

    let dir = tempfile::tempdir().unwrap();
    let server = SocketServer::start_in(engine.clone(), dir.path().to_path_buf()).await.unwrap();
    let mut waiter = Client::connect(&server.path).await.unwrap();
    let wait = Request::new(Actor::User, "bus.wait", json!({
        "events": ["guardrail.request_resolved"], "matching": {"request_id": id}, "timeout_ms": 5000,
    }));
    let wait = waiter.call(&wait, |_| {});
    let woken = tokio::time::timeout(std::time::Duration::from_secs(2), wait).await
        .expect("a wait begun after the answer returns at once").unwrap();
    let result = woken.result.unwrap();
    assert_eq!(result["timed_out"], false);
    assert_eq!(result["event"]["ev"], "guardrail.request_resolved");
    assert_eq!(result["event"]["payload"]["state"], "confirmed");
    assert_eq!(result["event"]["payload"]["scope"], "session");
    // A matching key the stored answer does not have still waits for a live event.
    let other = waiter.call(&Request::new(Actor::User, "bus.wait", json!({
        "events": ["guardrail.request_resolved"], "matching": {"request_id": id, "state": "rejected"},
        "timeout_ms": 1000,
    })), |_| {}).await.unwrap();
    assert_eq!(other.result.unwrap()["timed_out"], true);

    // The plan the agent checks before acting now agrees with the gate it will meet.
    let after = ok(engine, agent.clone(), "guardrail.explain", plan);
    assert_eq!(after["verdict"], "allow", "{after}");
    assert!(after["write_roots"].as_array().unwrap().iter().any(|root| root == sdk), "{after}");
    let granted = ok(engine, agent.clone(), "guardrail.request.get", json!({"request_id": id}));
    assert_eq!(granted["uses"], 0, "explaining a plan never uses a grant");
    assert!(call(engine, agent.clone(), "guardrail.gate", json!({
        "session": name, "kind": "write", "path": format!("{sdk}/licenses/x"), "new_text": "x\n",
    })).ok);
}

/// The default rules' gaps: every destructive remote push is the force rule, and the CLI's
/// pairing and cloud-link commands, which hand out phone tokens and the room secret, are the
/// user's to run. Ordinary pushes and the CLI's other commands stay an agent's.
#[test]
fn destructive_pushes_and_pairing_commands_are_refused_to_an_agent() {
    let fixture = Fixture::new();
    let engine = &fixture.engine;
    let session = fixture.session("builder", "claude");
    let name = session["name"].as_str().unwrap();
    let agent = Actor::agent(name);
    let check = |command: &str| ok(engine, agent.clone(), "guardrail.check",
        json!({"project_id": 1, "kind": "exec", "command": command}));
    for command in [
        "git push --mirror origin", "git push origin :main", "git push origin :refs/heads/main",
        "git push --delete origin main", "git push -d origin main", "git push origin -d main",
        "git push --force-with-lease origin main", "git push --force-with-lease=main:abc123 origin main",
        "git push --force-if-includes --force-with-lease origin main", "git push -f", "git push --prune origin",
        "git -C ../app push --mirror origin",
    ] {
        let verdict = check(command);
        assert_eq!(verdict["verdict"], "refuse", "{command}: {verdict}");
        assert_eq!(verdict["error"]["code"], "guardrail.command", "{command}: {verdict}");
    }
    for command in [
        "git push", "git push -u origin relay/x", "git push origin relay/x", "git push --set-upstream origin HEAD",
        "git push origin :", "git push --tags origin", "git push -o ci.skip origin relay/x", "git branch -d old",
    ] {
        assert_eq!(check(command)["verdict"], "allow", "{command} is an ordinary push");
    }
    for command in [
        "relay remote approve", "relay remote pair --no-confirm", "relay remote decline", "relay remote revoke abc",
        "relay remote via wss://example.test", "relay remote serve --pair --no-confirm", "relay --instance dev remote approve",
        "relay cloud connect my-bot.onrender.com", "relay cloud pair 'relay://x'", "relay cloud off",
        "\"$RELAY_BIN\" remote approve", "sh -c 'relay remote revoke abc'",
    ] {
        let verdict = check(command);
        assert_eq!(verdict["verdict"], "refuse", "{command}: {verdict}");
        assert_eq!(verdict["error"]["code"], "guardrail.self_approval", "{command}: {verdict}");
    }
    for command in [
        "relay remote devices", "relay cloud status", "relay cloud sync", "relay remote serve",
        "rg 'remote approve' docs", "relay q task.list",
    ] {
        assert_eq!(check(command)["verdict"], "allow", "{command} is not the user's alone");
    }
    // The person runs them as they like.
    let user = ok(engine, Actor::User, "guardrail.check", json!({"project_id": 1, "kind": "exec", "command": "relay remote approve"}));
    assert_eq!(user["verdict"], "allow");
}

/// The plugin layer (pictures-SPEC-v2 §4, C1): an op a plugin brings is offered to an agent and
/// callable by it only in a project with that plugin on; discovery (`callability`) and execution
/// (`authorize`) answer from one rule. The person is not gated.
#[test]
fn an_op_a_plugin_brings_exists_only_where_it_is_on() {
    use relay_bus::registry::{Callable, Registry};
    use relay_core::{guardrail, plugins, sessions};
    let fixture = Fixture::new();
    let engine = &fixture.engine;
    let session = fixture.session("builder", "claude");
    let name = session["name"].as_str().unwrap();
    let agent = Actor::agent(name);
    // The bundled design plugin stands in for one that brings ops: plugin.enable refuses an id
    // with no folder. A query and a mutation, since authorize answers queries early.
    let brought: Vec<_> = ["design.get", "design.create"]
        .into_iter()
        .map(|op| {
            let mut entry = Registry::global().get(op).unwrap().clone();
            entry.meta.plugin = Some("design");
            entry
        })
        .collect();
    let gate = |on: bool| {
        let conn = engine.store.lock();
        let row = sessions::by_name(&conn, name).unwrap();
        let cfg = guardrail::config(&conn, Some(1)).unwrap();
        let plugins_on = plugins::enabled_ids(&conn, 1).unwrap();
        assert_eq!(plugins_on.iter().any(|id| id == "design"), on);
        for entry in &brought {
            let (call, why) = guardrail::callability(entry, &agent, Some(&row.session), Some(&cfg), &plugins_on);
            let verdict = guardrail::authorize(&conn, row.session.id, entry, &json!({}));
            if on {
                assert_ne!(call, Callable::No, "{}: {why}", entry.name);
                assert!(verdict.is_ok(), "{}: {verdict:?}", entry.name);
            } else {
                assert_eq!((call, why.as_str()), (Callable::No, "plugin:design is off for this project"), "{}", entry.name);
                let refused = verdict.unwrap_err();
                assert_eq!((refused.kind, refused.code.as_str()), (ErrorKind::Refused, "plugin.off"), "{}", entry.name);
                assert_eq!(refused.details, Some(json!({"plugin": "design"})));
                assert_eq!(refused.message, "Design is off for this project");
                assert!(refused.hint.as_deref().unwrap().contains("switch on Design for this project"), "{:?}", refused.hint);
            }
            // The person, with no session, is not gated by plugins.
            assert_eq!(guardrail::callability(entry, &Actor::User, None, None, &[]).0, Callable::Yes);
        }
        // An op no plugin brings is untouched by the switch.
        let plain = Registry::global().get("design.get").unwrap();
        assert_ne!(guardrail::callability(plain, &agent, Some(&row.session), Some(&cfg), &plugins_on).0, Callable::No);
        assert!(guardrail::authorize(&conn, row.session.id, plain, &json!({})).is_ok());
    };
    gate(false);
    ok(engine, Actor::User, "plugin.enable", json!({"plugin_id": "design", "project_id": 1, "enabled": true}));
    gate(true);
    ok(engine, Actor::User, "plugin.enable", json!({"plugin_id": "design", "project_id": 1, "enabled": false}));
    gate(false);

    // The real thing: the Pictures ops in a project without Pictures. bus.ops offers them with
    // the reason, execution refuses before the handler (or its absence) is reached, and the
    // person's call is not refused for the plugin.
    let ops = ok(engine, agent.clone(), "bus.ops", json!({}));
    let rows = ops["ops"].as_array().unwrap();
    for op in ["design.picture.search", "design.picture.thumb", "design.picture.add"] {
        let row = rows.iter().find(|row| row["name"] == op).unwrap_or_else(|| panic!("{op} is listed"));
        assert_eq!((row["call"].as_str(), row["why"].as_str()), (Some("no"), Some("plugin:pictures is off for this project")), "{op}");
    }
    for op in ["design.picture.key_set", "design.picture.key_remove"] {
        assert!(!rows.iter().any(|row| row["name"] == op), "{op} is the person's alone");
    }
    // Every op a plugin brings, whatever it is and however it is registered, where its plugin is
    // off for this project (the fixture's has none on).
    {
        let conn = engine.store.lock();
        let row = sessions::by_name(&conn, name).unwrap();
        let cfg = guardrail::config(&conn, Some(1)).unwrap();
        let brought: Vec<_> = Registry::global().entries().iter().filter(|entry| entry.meta.plugin.is_some()).collect();
        assert!(brought.len() >= 3, "the Pictures ops carry their plugin");
        for entry in brought {
            let plugin = entry.meta.plugin.unwrap();
            if entry.meta.actors.admits(&agent) {
                assert_eq!(guardrail::callability(entry, &agent, Some(&row.session), Some(&cfg), &[]).0, Callable::No, "{}", entry.name);
                let refused = guardrail::authorize(&conn, row.session.id, entry, &json!({})).unwrap_err();
                assert_eq!((refused.code.as_str(), refused.details), ("plugin.off", Some(json!({"plugin": plugin}))), "{}", entry.name);
            }
        }
    }
    let thumb = json!({"design_id": 1, "id": "nope"});
    let refused = call(engine, agent.clone(), "design.picture.thumb", thumb.clone());
    assert_eq!((error(&refused).code.as_str(), &error(&refused).details), ("plugin.off", &Some(json!({"plugin": "pictures"}))));
    let mine = ok(engine, Actor::User, "bus.ops", json!({}));
    let row = mine["ops"].as_array().unwrap().iter().find(|row| row["name"] == "design.picture.thumb").unwrap();
    assert_eq!(row["call"], "yes");
    let person = call(engine, Actor::User, "design.picture.thumb", thumb);
    assert_ne!(person.error.as_ref().map(|err| err.code.as_str()), Some("plugin.off"));
}

/// E-agent-2: a session name that is no live session of the agent's project is a typo or a stale
/// name, and is answered as one, with the nearest name of that project; `actor.scope` is kept
/// for a real session of the project that the agent may not act on.
#[test]
fn a_session_name_that_names_nobody_here_is_not_found_not_a_permission_problem() {
    let fixture = Fixture::new();
    let engine = &fixture.engine;
    let me = fixture.session("builder", "claude");
    let peer = fixture.session("builder", "codex");
    let (me, peer) = (me["name"].as_str().unwrap(), peer["name"].as_str().unwrap());
    let agent = Actor::agent(me);

    let typo = call(engine, agent.clone(), "design.list", json!({"session": "typo"}));
    let typo = error(&typo);
    assert_eq!((typo.kind, typo.code.as_str()), (ErrorKind::NotFound, "session.not_found"), "{}", typo.message);
    assert!(typo.message.contains("\"typo\"") && typo.message.contains("this project"), "{}", typo.message);
    // A slip of one letter is pointed at the session it was meant to be.
    let slip = format!("{}{}", &peer[..peer.len() - 1], if peer.ends_with('x') { 'y' } else { 'x' });
    let slipped = call(engine, agent.clone(), "design.list", json!({"session": slip}));
    assert_eq!(error(&slipped).code, "session.not_found");
    assert_eq!(error(&slipped).hint.as_deref(), Some(format!("did you mean {peer:?}?").as_str()));
    // A mutation naming nobody is a typo too, not a refusal to act on someone else.
    let mutation = call(engine, agent.clone(), "session.intent", json!({"session": "typo", "text": "x"}));
    assert_eq!(error(&mutation).code, "session.not_found");

    // A real peer: reads reach it, writes stay the peer's own.
    ok(engine, agent.clone(), "design.list", json!({"session": peer}));
    let theirs = call(engine, agent.clone(), "session.intent", json!({"session": peer, "text": "x"}));
    assert_eq!(error(&theirs).code, "actor.scope");
    ok(engine, agent.clone(), "design.list", json!({"session": me}));

    // Another project's session is not this agent's to know of: not found here, and no hint
    // names it.
    let other = fixture.repo.parent().unwrap().join("other");
    committed_repo(&other, &[("README.md", "other\n")]);
    ok(engine, Actor::User, "project.add", json!({"workspace_id": 1, "path": other}));
    let outsider = ok(engine, Actor::User, "session.create", json!({"project_id": 2, "provider": "claude", "role": "builder"}));
    let outsider = outsider["name"].as_str().unwrap();
    for (op, payload) in [
        ("design.list", json!({"session": outsider})),
        ("session.intent", json!({"session": outsider, "text": "x"})),
    ] {
        let refused = call(engine, agent.clone(), op, payload);
        let refused = error(&refused);
        assert_eq!(refused.code, "session.not_found", "{op}");
        assert!(!refused.hint.as_deref().unwrap_or("").contains(outsider), "{op}: {:?}", refused.hint);
    }
}
