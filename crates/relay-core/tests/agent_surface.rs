//! The agent-facing surface, exercised the way an agent meets it: as a bound agent actor,
//! through the bus, with nothing read out of the source. Each test pins one finding from the
//! 2026-08-19 agent surface audit (docs/DECISIONS.md D101 … D111).

mod common;

use common::{call_as as call, committed_repo, engine_with_project, git, ok_as as ok};
use relay_bus::Actor;
use relay_core::engine::Engine;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;

/// Unlike `common::refused`, takes the request and names the op that should have refused.
fn refusal(engine: &Engine, actor: Actor, op: &str, payload: Value) -> relay_bus::error::BusError {
    call(engine, actor, op, payload)
        .into_result()
        .expect_err(&format!("{op} was expected to refuse"))
}

struct Fixture {
    _root: tempfile::TempDir,
    engine: Arc<Engine>,
    builder: Value,
    peer: Value,
    /// A builder in a second project — the cross-project boundary under test.
    outsider: Value,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let ws = root.path().join("ws");
        let first = ws.join("first");
        let second = ws.join("second");
        committed_repo(&first, &[("src/lib.rs", "pub fn one() -> i32 { 1 }\n")]);
        committed_repo(&second, &[("src/lib.rs", "pub fn one() -> i32 { 1 }\n")]);

        let engine = engine_with_project(root.path(), &ws, &first);
        ok(&engine, Actor::User, "project.add", json!({"workspace_id": 1, "path": second}));

        let builder = ok(&engine, Actor::User, "session.create",
            json!({"project_id": 1, "provider": "claude", "role": "builder"}));
        let peer = ok(&engine, Actor::User, "session.create",
            json!({"project_id": 1, "provider": "codex", "role": "builder"}));
        let outsider = ok(&engine, Actor::User, "session.create",
            json!({"project_id": 2, "provider": "claude", "role": "builder"}));
        Self { _root: root, engine, builder, peer, outsider }
    }

    fn me(&self) -> Actor {
        Actor::agent(self.builder["name"].as_str().unwrap())
    }
    fn name(&self) -> &str {
        self.builder["name"].as_str().unwrap()
    }
    fn peer_name(&self) -> &str {
        self.peer["name"].as_str().unwrap()
    }
    fn worktree(&self) -> &str {
        self.builder["worktree"].as_str().unwrap()
    }
}

/// F3: `bus.ops` used to report layer 1 only, so it promised a builder ten times the write
/// surface it has. Every row now carries the verdict from all three layers.
#[test]
fn bus_ops_reports_every_gating_layer_not_just_the_first() {
    let f = Fixture::new();
    let listed = ok(&f.engine, f.me(), "bus.ops", json!({}));
    let ops = listed["ops"].as_array().unwrap();
    let row = |name: &str| {
        ops.iter()
            .find(|op| op["name"] == name)
            .unwrap_or_else(|| panic!("{name} is missing from bus.ops"))
            .clone()
    };

    assert_eq!(row("mailbox.send")["call"], "yes");
    assert!(row("mailbox.send")["why"].as_str().unwrap().contains("builder"));

    // Advertised by the schema, refused by the allowlist. Both statements now come from the
    // same place, so they can no longer disagree.
    assert_eq!(row("task.create")["call"], "no", "task.create is outside the builder allowlist");
    assert_eq!(row("git.commit")["call"], "no");
    assert_eq!(row("file.write")["call"], "no");
    assert!(row("file.write")["why"].as_str().unwrap().contains("bus_writes"));

    // Layer 2: callable, but only ever against this session.
    assert_eq!(row("session.brief")["call"], "self_only");

    // `user_only` never reaches an agent's listing at all.
    assert!(!ops.iter().any(|op| op["name"] == "session.spawn"));

    for name in ["task.create", "git.commit", "file.write"] {
        let error = refusal(&f.engine, f.me(), name, match name {
            "task.create" => json!({"project_id": 1, "title": "x"}),
            "git.commit" => json!({"project_id": 1, "message": "x"}),
            _ => json!({"project_id": 1, "path": "src/lib.rs", "text": "x"}),
        });
        assert_eq!(error.code, "actor.allowlist", "{name} claims No but refuses differently");
    }
}

/// Role prompts and authorization are maintained in different modules. Any operation named in
/// a prompt must survive the complete actor/role policy for that same role.
#[test]
fn role_instructions_only_name_operations_that_role_can_call() {
    let f = Fixture::new();
    let reviewer = ok(&f.engine, Actor::User, "session.create", json!({
        "project_id": 1, "provider": "claude", "role": "reviewer", "pair_with": f.name()
    }));
    let docs = ok(&f.engine, Actor::User, "session.create", json!({
        "project_id": 1, "provider": "claude", "role": "docs"
    }));

    for (session, role) in [
        (f.name(), relay_bus::types::Role::Builder),
        (reviewer["name"].as_str().unwrap(), relay_bus::types::Role::Reviewer),
        (docs["name"].as_str().unwrap(), relay_bus::types::Role::Docs),
    ] {
        let instructions = relay_core::providers::role_instructions(role);
        let listed = ok(&f.engine, Actor::agent(session), "bus.ops", json!({}));
        let rows = listed["ops"].as_array().unwrap();
        let mentioned: Vec<&str> = relay_bus::registry::Registry::global().entries().iter()
            .map(|entry| entry.name)
            .filter(|name| instructions.contains(name))
            .collect();
        assert!(mentioned.len() >= 10, "{role:?} contract test found too few named operations");
        for name in mentioned {
            let op = rows.iter().find(|op| op["name"] == name)
                .unwrap_or_else(|| panic!("{role:?} is told to call hidden operation {name}"));
            assert_eq!(op["implemented"], true, "{role:?} is told to call unimplemented {name}");
            assert_ne!(op["call"], "no", "{role:?} is told to call forbidden {name}: {}", op["why"]);
        }
    }
}

/// F4: the only finding that produced silently wrong output. An omitted `worktree` meant the
/// project root, so an agent reviewing "its" diff read a tree it had never touched.
#[test]
fn omitted_worktree_means_the_callers_own_tree() {
    let f = Fixture::new();
    let mine = ok(&f.engine, f.me(), "git.status", json!({"project_id": 1}));
    assert_eq!(
        mine["branch"], f.builder["branch"],
        "an agent's default worktree must be its own, not the project root",
    );

    // The project root is still reachable — by asking for it, out loud.
    let root = ok(&f.engine, f.me(), "git.status", json!({"project_id": 1, "worktree": "@project"}));
    assert_eq!(root["branch"], "main");

    // The user, who has no session, still defaults to the project root.
    assert_eq!(ok(&f.engine, Actor::User, "git.status", json!({"project_id": 1}))["branch"], "main");

    // file.* draws the same line: a file written into the session worktree is visible to a
    // default-worktree read and absent from the project root.
    std::fs::write(Path::new(f.worktree()).join("only-here.txt"), "x\n").unwrap();
    let mine = ok(&f.engine, f.me(), "file.tree", json!({"project_id": 1}));
    let names: Vec<&str> = mine["entries"].as_array().unwrap().iter()
        .map(|entry| entry["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"only-here.txt"), "file.tree read the wrong tree: {names:?}");
}

/// F5: the harness mandates a scratchpad; the guardrail refused it, and called a deliberate
/// policy decision "unavailable". Scratch space is not a repo-integrity concern.
#[test]
fn scratch_writes_are_allowed_and_unknown_roots_are_refused_not_broken() {
    let f = Fixture::new();
    let scratch = std::env::temp_dir().join("relay-surface-test/scratch.md");
    let allowed = ok(&f.engine, f.me(), "guardrail.check", json!({
        "project_id": 1, "kind": "write",
        "path": scratch.display().to_string(), "new_text": "scratch\n",
    }));
    assert_eq!(allowed["verdict"], "allow", "the mandated scratchpad must be writable");

    let elsewhere = ok(&f.engine, f.me(), "guardrail.check", json!({
        "project_id": 1, "kind": "write",
        "path": "/etc/relay-surface-test.conf", "new_text": "no\n",
    }));
    assert_eq!(elsewhere["verdict"], "refuse");
    let error = elsewhere["error"].clone();
    assert_eq!(error["kind"], "refused", "a policy decision is a refusal, not an outage");
    assert_eq!(error["code"], "guardrail.write_root");
    assert!(
        error["message"].as_str().unwrap().contains(f.worktree()),
        "the refusal must name where the agent may write: {}", error["message"],
    );

    // The worktree is still the ordinary case, absolute or relative.
    let inside = ok(&f.engine, f.me(), "guardrail.check", json!({
        "project_id": 1, "kind": "write",
        "path": Path::new(f.worktree()).join("src/new.rs").display().to_string(),
        "new_text": "fn new() {}\n",
    }));
    assert_eq!(inside["verdict"], "allow");
}

/// F6: `denied_commands` matched raw substrings, so an agent could not even *ask* whether a
/// command was allowed — the question contained the answer.
#[test]
fn denied_commands_match_argv_not_quoted_data() {
    let f = Fixture::new();
    let check = |command: &str| {
        ok(&f.engine, f.me(), "guardrail.check",
            json!({"project_id": 1, "kind": "exec", "command": command}))["verdict"]
            .as_str().unwrap().to_string()
    };
    let denied = format!("{} -rf", "rm");

    assert_eq!(check(&format!("{denied} /tmp/whatever")), "refuse", "actually running it is denied");
    assert_eq!(check("git push --force"), "refuse");
    assert_eq!(check(&format!("echo \"{denied} /\"")), "allow", "a quoted mention is data");
    assert_eq!(
        check(&format!("relay q guardrail.check '{{\"command\":\"{denied} /\"}}'")),
        "allow",
        "asking whether a command is allowed must never be the blocked act",
    );
    assert_eq!(
        check(&format!("relay q guardrail.check && {denied} /tmp/x")),
        "refuse",
        "the exemption covers the dry run, not whatever is chained after it",
    );
    assert_eq!(check("git status && cargo test"), "allow");
}

/// F10: a send reported `acked_at: null` and nothing else, and the sender could not see its
/// own sent mail at all.
#[test]
fn a_send_says_where_it_went_and_the_sender_can_look_it_up() {
    let f = Fixture::new();
    let sent = ok(&f.engine, f.me(), "mailbox.send",
        json!({"project_id": 1, "to": f.peer_name(), "text": "ping"}));
    assert_eq!(sent["message"]["text"], "ping");
    let addressed: Vec<&str> = sent["recipients"].as_array().unwrap().iter()
        .map(|r| r["session"].as_str().unwrap()).collect();
    assert_eq!(addressed, vec![f.peer_name()]);
    assert!(!sent["delivery"].as_str().unwrap().is_empty());

    let outbox = ok(&f.engine, f.me(), "mailbox.outbox", json!({"project_id": 1}));
    let entries = outbox["sent"].as_array().unwrap();
    assert_eq!(entries.len(), 1, "the sender must be able to see what it sent");
    assert_eq!(entries[0]["message"]["text"], "ping");
    assert_eq!(entries[0]["recipients"][0]["session"], f.peer_name());
    assert!(entries[0]["recipients"][0]["acked_at"].is_null());

    // The recipient's outbox is its own, not a view of everyone's traffic.
    let peer_outbox = ok(&f.engine, Actor::agent(f.peer_name()), "mailbox.outbox", json!({"project_id": 1}));
    assert!(peer_outbox["sent"].as_array().unwrap().is_empty());
}

/// F11: `session.list` returned every session in every project while `session.get` refused a
/// peer in the caller's own. One boundary, drawn in one place: the caller's project.
#[test]
fn session_visibility_stops_at_the_project_and_stops_consistently() {
    let f = Fixture::new();
    let listed = ok(&f.engine, f.me(), "session.list", json!({}));
    let names: Vec<&str> = listed["sessions"].as_array().unwrap().iter()
        .map(|s| s["name"].as_str().unwrap()).collect();
    assert!(names.contains(&f.name()));
    assert!(names.contains(&f.peer_name()));
    assert!(
        !names.contains(&f.outsider["name"].as_str().unwrap()),
        "another project's sessions are not this agent's business: {names:?}",
    );

    // Asking for someone else's project is refused rather than quietly answered.
    assert_eq!(refusal(&f.engine, f.me(), "session.list", json!({"project_id": 2})).code, "actor.scope");

    // …and `session.get` admits exactly what `session.list` showed: a session it did not list
    // is not found here (E-agent-2), as a typo is.
    assert_eq!(ok(&f.engine, f.me(), "session.get", json!({"session": f.peer_name()}))["name"], f.peer_name());
    assert_eq!(
        refusal(&f.engine, f.me(), "session.get",
            json!({"session": f.outsider["name"].as_str().unwrap()})).code,
        "session.not_found",
    );

    // The user still sees the whole fleet.
    assert_eq!(ok(&f.engine, Actor::User, "session.list", json!({}))["sessions"].as_array().unwrap().len(), 3);
}

/// RA-208, the read half: the board and mail refused another project, but its source, diffs,
/// history, worktrees and guardrails answered any agent that named it. One check in `authorize`
/// now draws the line for every op.
#[test]
fn an_agent_reads_nothing_of_another_project() {
    let f = Fixture::new();
    let other = json!({"project_id": 2});
    for op in [
        "file.tree", "git.status", "git.log", "git.branches", "worktree.list", "worktree.disk",
        "guardrail.config.get", "guardrail.config.layers", "project.get", "device.run.list", "skill.list", "plugin.list",
    ] {
        assert_eq!(refusal(&f.engine, f.me(), op, other.clone()).code, "actor.scope", "{op}");
    }
    for (op, payload) in [
        ("file.read", json!({"project_id": 2, "path": "src/lib.rs"})),
        ("file.search", json!({"project_id": 2, "query": "one"})),
        ("git.diff", json!({"project_id": 2})),
        ("git.show", json!({"project_id": 2, "sha": "HEAD"})),
        ("file.write", json!({"project_id": 2, "path": "src/x.rs", "text": "x"})),
    ] {
        assert_eq!(refusal(&f.engine, f.me(), op, payload).code, "actor.scope", "{op}");
    }
    // Its own project, named or left for the engine to fill in, still answers.
    for payload in [json!({"project_id": 1}), json!({})] {
        ok(&f.engine, f.me(), "file.tree", payload.clone());
        ok(&f.engine, f.me(), "git.log", payload.clone());
        ok(&f.engine, f.me(), "guardrail.config.get", payload);
    }
    assert!(ok(&f.engine, f.me(), "file.read", json!({"path": "src/lib.rs"}))["text"].as_str().unwrap().contains("one"));
    // A person at the PC reads every project.
    ok(&f.engine, Actor::User, "file.tree", json!({"project_id": 2}));
    ok(&f.engine, Actor::User, "git.log", json!({"project_id": 2}));
}

/// F12: an agent already *is* a project and a session; every call began with guess-and-retry
/// because it had to say so anyway.
#[test]
fn identity_fields_are_filled_in_from_the_authenticated_session() {
    let f = Fixture::new();
    // `project_id` is required by mailbox.list and was the commonest retry.
    let inbox = ok(&f.engine, f.me(), "mailbox.list", json!({}));
    assert!(inbox["messages"].as_array().unwrap().is_empty());

    // `session.peers` takes exactly one of two optional targets; for an agent, neither means
    // "my project, minus me".
    let peers = ok(&f.engine, f.me(), "session.peers", json!({}));
    let names: Vec<&str> = peers["peers"].as_array().unwrap().iter()
        .map(|p| p["session"].as_str().unwrap()).collect();
    assert_eq!(names, vec![f.peer_name()]);

    // A required `session` field is filled the same way.
    ok(&f.engine, f.me(), "session.report", json!({"kind": "tool_use"}));
    assert_eq!(ok(&f.engine, f.me(), "session.get", json!({"session": f.name()}))["state"], "running");

    // Optional either/or fields are left alone: filling one would break the other.
    assert_eq!(
        refusal(&f.engine, f.me(), "notes.append", json!({"text": "x", "note_id": 999})).code,
        "notes.not_found",
    );

    // An explicit value always wins over the fill-in.
    assert_eq!(refusal(&f.engine, f.me(), "mailbox.list", json!({"project_id": 2})).code, "actor.scope");
}

/// F1 + F8 + F9: the two disclosure surfaces every agent meets on turn one.
#[test]
fn bootstrap_and_the_brief_disclose_what_the_engine_already_knows() {
    let f = Fixture::new();
    let boot = ok(&f.engine, f.me(), "session.bootstrap", json!({}));
    let peers: Vec<&str> = boot["peers"].as_array().unwrap().iter()
        .map(|p| p["session"].as_str().unwrap()).collect();
    assert_eq!(peers, vec![f.peer_name()], "the peer table is one query away and belongs here");
    assert_eq!(boot["project_id"], 1);
    assert!(boot["comms"].as_str().unwrap().contains("mailbox.send"));
    assert!(boot["comms"].as_str().unwrap().contains("{\"to\":\"*\",\"text\":\"...\"}"));
    assert!(boot["discovery"].as_str().unwrap().contains("bus.ops"));
    assert!(boot["discovery"].as_str().unwrap().contains("bus.schema"));
    assert!(boot["discovery"].as_str().unwrap().contains("$RELAY_BIN schema <op>"));
    assert!(boot["discovery"].as_str().unwrap().contains("project_id"));
    assert_eq!(boot["guardrails"]["dry_run"], "guardrail.check");
    assert!(boot["guardrails"]["denied_commands"].as_array().unwrap().len() >= 4);

    let can_call: Vec<&str> = boot["can_call"].as_array().unwrap().iter()
        .map(|op| op.as_str().unwrap()).collect();
    assert!(can_call.contains(&"mailbox.outbox"));
    assert!(can_call.contains(&"session.claim"));
    assert!(can_call.contains(&"session.intent"));
    assert!(can_call.contains(&"session.release"));
    assert!(!can_call.contains(&"task.create"));
    // `can_call` and `bus.ops` are the same answer computed once.
    let listed = ok(&f.engine, f.me(), "bus.ops", json!({}));
    let from_ops: Vec<String> = listed["ops"].as_array().unwrap().iter()
        .filter(|op| op["call"] != "no" && op["implemented"] == true)
        .map(|op| op["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(can_call, from_ops, "bootstrap and bus.ops must not drift");

    let schema = ok(&f.engine, f.me(), "bus.schema", json!({"op": "mailbox.send"}));
    assert_eq!(schema["schema"]["op"], "mailbox.send");
    assert!(schema["schema"]["payload"]["required"].as_array().unwrap().iter()
        .any(|field| field == "text"), "the advertised schema must expose exact payload fields");

    let done_schema = ok(&f.engine, f.me(), "bus.schema", json!({"op": "session.done"}));
    let done_fields = done_schema["schema"]["payload"]["properties"].as_object().unwrap();
    assert!(!done_fields.contains_key("task"));
    assert!(!done_fields.contains_key("task_id"));

    let broadcast = ok(&f.engine, f.me(), "mailbox.send", json!({"to": "*", "text": "..."}));
    let recipients: Vec<&str> = broadcast["recipients"].as_array().unwrap().iter()
        .map(|recipient| recipient["session"].as_str().unwrap()).collect();
    assert_eq!(recipients, vec![f.peer_name()], "the documented broadcast must stay agent-callable and project-scoped");

    // The compact brief is what gets injected; it carries the peers and never the assignment.
    let brief = ok(&f.engine, f.me(), "session.brief", json!({"session": f.name()}));
    let compact = brief["compact"].as_str().unwrap();
    assert!(compact.contains(f.peer_name()), "the injected half must carry the peer table");
    assert!(compact.contains("mailbox.send"));
    // With nothing installed the skills section says so; when skills exist it names them and
    // the folders they are registered in, and still never inlines a body (see awareness.rs).
    assert!(
        compact.contains("No enabled Relay skills."),
        "the injected half goes silent about skills instead of stating where they stand",
    );
    assert!(!compact.contains("Launch assignment"), "the assignment never enters an argv-bound string");
}

/// A percentage of a very short file measures nothing: rewriting a one-line scratch file is
/// 100% removed and was held every time, while the absolute rule it tripped past was 50 lines.
#[test]
fn short_files_are_not_destructive_just_because_the_share_is_large() {
    let f = Fixture::new();
    let write = |path: &str, text: &str| {
        ok(&f.engine, f.me(), "guardrail.check",
            json!({"project_id": 1, "kind": "write", "path": path, "new_text": text}))["verdict"]
            .as_str().unwrap().to_string()
    };
    let worktree = Path::new(f.worktree());

    // One line replaced by another: 100% of the file, one line of loss.
    std::fs::write(worktree.join("stub.py"), "# placeholder\n").unwrap();
    assert_eq!(write("stub.py", "print('real content')\n"), "allow");

    // A handful of lines, wholly rewritten: still nowhere near the absolute limit.
    std::fs::write(worktree.join("small.toml"), "a = 1\nb = 2\nc = 3\n").unwrap();
    assert_eq!(write("small.toml", "x = 9\ny = 8\n"), "allow");

    // A long file, gutted: exactly what the rule is for, and still caught.
    let long: String = (0..200).map(|line| format!("line-{line}\n")).collect();
    std::fs::write(worktree.join("long.rs"), &long).unwrap();
    assert_eq!(write("long.rs", "fn main() {}\n"), "hold");

    // The percentage rule still bites above the floor, below the absolute limit: 40 lines of
    // a 60-line file is 67% — two thirds gone, and only 40 lines, so the count alone misses it.
    let sixty: String = (0..60).map(|line| format!("line-{line}\n")).collect();
    let kept: String = (0..20).map(|line| format!("line-{line}\n")).collect();
    std::fs::write(worktree.join("sixty.rs"), &sixty).unwrap();
    assert_eq!(write("sixty.rs", &kept), "hold");

    // A brand-new file removes nothing.
    assert_eq!(write("brand-new.md", "# hello\n"), "allow");
}

/// Volume is a proxy for harm; what matters is whether the work can come back. A committed,
/// unmodified file is one `git checkout --` from being restored, so gutting it is not the
/// event worth interrupting a person for — gutting uncommitted work is.
#[test]
fn a_large_rewrite_holds_only_when_the_content_would_be_lost() {
    let f = Fixture::new();
    let worktree = Path::new(f.worktree());
    let long: String = (0..200).map(|line| format!("line-{line}\n")).collect();
    let check = |path: &str| {
        ok(&f.engine, f.me(), "guardrail.check",
            json!({"project_id": 1, "kind": "write", "path": path, "new_text": "fn main() {}\n"}))
    };

    // Uncommitted: the only copy of these 200 lines is the file about to be overwritten.
    std::fs::write(worktree.join("scratch.rs"), &long).unwrap();
    assert_eq!(check("scratch.rs")["verdict"], "hold", "untracked work would be lost");

    // Committed and clean: git has it.
    std::fs::write(worktree.join("tracked.rs"), &long).unwrap();
    git(worktree, &["add", "tracked.rs"]);
    // The session worktree carries Relay's own pre-commit hook, which wants a live socket.
    git(worktree, &["commit", "--no-verify", "-qm", "add tracked"]);
    assert_eq!(check("tracked.rs")["verdict"], "allow", "git can restore a committed file");

    // Committed but since modified: the modifications exist nowhere else.
    std::fs::write(worktree.join("tracked.rs"), format!("{long}extra\n")).unwrap();
    assert_eq!(check("tracked.rs")["verdict"], "hold", "uncommitted edits would be lost");

    // Ignored: as often `.env` or local config as build output, and git cannot restore it (RA-094).
    std::fs::write(worktree.join(".gitignore"), "generated.rs\n").unwrap();
    std::fs::write(worktree.join("generated.rs"), &long).unwrap();
    assert_eq!(check("generated.rs")["verdict"], "hold", "git cannot bring an ignored file back");

    // The escape is a setting, not a law. Back to the committed content first, which the escape
    // allows, so the hold below can only come from switching it off.
    std::fs::write(worktree.join("tracked.rs"), &long).unwrap();
    assert_eq!(check("tracked.rs")["verdict"], "allow", "clean again, so git can restore it");
    ok(&f.engine, Actor::User, "guardrail.config.set",
        json!({"patch": {"destructive_write": {"allow_if_recoverable": false}}}));
    assert_eq!(check("tracked.rs")["verdict"], "hold", "the escape can be switched off");
}

/// RA-009: confirming a held agent `file.write` replays it as that agent, session included. The
/// replay used to run with the confirmer's session (none, for the user), so a write that named
/// no `worktree` resolved to the project's primary checkout instead of the agent's own.
#[test]
fn a_confirmed_agent_write_lands_in_the_agents_worktree() {
    let f = Fixture::new();
    let writer = ok(&f.engine, Actor::User, "session.create",
        json!({"project_id": 1, "provider": "claude", "role": "builder", "bus_writes": true}));
    let agent = Actor::agent(writer["name"].as_str().unwrap());
    let worktree = Path::new(writer["worktree"].as_str().unwrap()).to_path_buf();
    let primary = Path::new(&ok(&f.engine, Actor::User, "project.get", json!({"project_id": 1}))["path"]
        .as_str().unwrap().to_string()).to_path_buf();
    assert_ne!(worktree, primary);

    // Untracked, so git cannot restore it: the rewrite is held as a destructive write.
    let long: String = (0..200).map(|line| format!("line-{line}\n")).collect();
    std::fs::write(worktree.join("scratch.rs"), &long).unwrap();
    let held = refusal(&f.engine, agent, "file.write",
        json!({"project_id": 1, "path": "scratch.rs", "text": "fn main() {}\n"}));
    assert_eq!(held.code, "guardrail.destructive_write");
    let hold_id = held.confirm.as_ref().unwrap().payload["hold_id"].as_i64().unwrap();

    let confirmed = ok(&f.engine, Actor::User, "guardrail.confirm", json!({"hold_id": hold_id}));
    assert_eq!(confirmed["outcome"]["ok"], true, "{}", confirmed["outcome"]);
    assert_eq!(std::fs::read_to_string(worktree.join("scratch.rs")).unwrap(), "fn main() {}\n");
    assert!(!primary.join("scratch.rs").exists(), "the replay wrote into the primary checkout");
}
