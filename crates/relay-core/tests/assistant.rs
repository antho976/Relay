//! Full access (docs/THREADS.md, "Full access"): a thread's agent with Claude Code's own tools and
//! every Relay op, which asks the person before each change, against a stand-in Claude that
//! speaks the permission protocol the real one does (`--permission-prompt-tool stdio`, checked
//! with Claude Code 2.1.302).

mod common;

use common::{code, engine, ok, wait_until};
use relay_bus::{Actor, Request, Response};
use relay_core::engine::{Door, Engine};
use serde_json::{json, Value};
use std::path::Path;

/// A Claude that logs its arguments (`args.log`). For each message it says it started; a message
/// with `RUNIT` asks to run a shell command that makes `made`, and makes it once allowed; one with
/// `CALLRELAY` asks for each tool in `tools.json` in turn, then runs `client.py` from inside its
/// own tree, as Relay's MCP server would. Each answer it is given goes to `responses.log`.
const FAKE: &str = r#"#!/usr/bin/env python3
import json, os, subprocess, sys
D = "DIR"
with open(D + "/args.log", "a") as f:
    f.write("OPS=%s %s\n" % (os.environ.get("RELAY_MCP_OPS", "<unset>"), " ".join(sys.argv[1:])))
def out(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()
def log(name, obj):
    with open(D + "/" + name, "a") as f:
        f.write(json.dumps(obj) + "\n")
asked = 0
def ask(tool, given):
    global asked
    asked += 1
    rid = "req-%d" % asked
    out({"type": "control_request", "request_id": rid, "request": {"subtype": "can_use_tool", "tool_name": tool,
         "display_name": tool, "input": given, "description": given.get("description", ""), "tool_use_id": "tu%d" % asked}})
    while True:
        line = sys.stdin.readline()
        if not line:
            sys.exit(0)
        msg = json.loads(line)
        if msg.get("type") == "control_response" and msg["response"]["request_id"] == rid:
            log("responses.log", {"tool": tool, "response": msg["response"]})
            return msg["response"]["response"]
while True:
    line = sys.stdin.readline()
    if not line:
        break
    text = line
    out({"type": "system", "subtype": "init", "session_id": "fake-session-1",
         "mcp_servers": [{"name": "relay", "status": "connected"}], "tools": ["Bash", "mcp__relay__thread_list"]})
    if "RUNIT" in text:
        given = {"command": "touch " + D + "/made", "description": "Make a file"}
        out({"type": "assistant", "message": {"role": "assistant", "content": [{"type": "tool_use", "id": "tu", "name": "Bash", "input": given}]}, "parent_tool_use_id": None})
        if ask("Bash", given).get("behavior") == "allow":
            open(D + "/made", "w").close()
    if "CALLRELAY" in text:
        for tool in json.load(open(D + "/tools.json")):
            ask(tool, {})
        subprocess.run([sys.executable, D + "/client.py", D])
    out({"type": "assistant", "message": {"role": "assistant", "content": [{"type": "text", "text": "Done"}]}, "parent_tool_use_id": None})
    out({"type": "result", "subtype": "success", "is_error": False, "result": "Done"})
"#;

/// Sends each `[op, payload]` in `calls.json` to the socket named in `sock`, as Relay's MCP
/// server does for a thread's agent, and writes the answers to `answers.json`.
const CLIENT: &str = r#"
import json, os, socket, sys, uuid
d = sys.argv[1]
s = socket.socket(socket.AF_UNIX)
s.connect(open(d + "/sock").read())
f = s.makefile("rw")
answers = []
for op, payload in json.load(open(d + "/calls.json")):
    f.write(json.dumps({"v": 1, "id": str(uuid.uuid4()), "actor": "user", "op": op, "payload": payload}) + "\n")
    f.flush()
    answers.append(json.loads(f.readline()))
with open(d + "/answers.tmp", "w") as out:
    json.dump(answers, out)
os.replace(d + "/answers.tmp", d + "/answers.json")
"#;

fn python() -> bool {
    let found = std::process::Command::new("python3").args(["-c", ""]).status().is_ok_and(|s| s.success());
    if !found {
        eprintln!("skipping: no python3 to stand in for Claude");
    }
    found
}

fn with_fake(engine: &Engine, dir: &Path) {
    let script = dir.join("claude");
    std::fs::write(&script, FAKE.replace("DIR", &dir.display().to_string())).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    std::fs::write(dir.join("client.py"), CLIENT).unwrap();
    ok(engine, "settings.set", json!({"path": "providers.claude.path", "value": script}));
}

/// What thread `thread`'s agent sends; the socket door would name the thread from its process.
fn from_agent(engine: &Engine, thread: &Value, op: &str, payload: Value) -> Response {
    engine.dispatch_from_thread(Request::new(Actor::User, op, payload), Door::InProcess, thread.as_i64().unwrap())
}

fn pending(engine: &Engine, thread: &Value) -> Vec<Value> {
    ok(engine, "thread.ask.list", json!({"thread_id": thread, "pending": true}))["asks"].as_array().unwrap().clone()
}

fn responses(dir: &Path) -> Vec<Value> {
    std::fs::read_to_string(dir.join("responses.log")).unwrap_or_default().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

fn idle(engine: &Engine, thread: &Value) {
    wait_until("the agent's turn to end", || ok(engine, "thread.get", json!({"id": thread}))["thread"]["working"] == false);
}

fn full_thread(engine: &Engine) -> Value {
    let created = ok(engine, "thread.create", json!({}));
    let thread = ok(engine, "thread.access", json!({"id": created["id"], "access": "full"}));
    assert_eq!(thread["access"], "full");
    created["id"].clone()
}

#[test]
fn full_access_is_the_persons_to_give_and_its_agent_asks_before_each_command() {
    if !python() {
        return;
    }
    let engine = engine();
    let dir = tempfile::tempdir().unwrap();
    with_fake(&engine, dir.path());
    let created = ok(&engine, "thread.create", json!({}));
    let id = created["id"].clone();
    assert_eq!(created["access"], "data", "a thread starts with its data");
    assert_eq!(code(engine.dispatch(Request::new(Actor::User, "thread.access", json!({"id": id, "access": "root"})), Door::InProcess)), "thread.invalid");
    // No thread's agent gives itself, or any thread, more.
    assert_eq!(code(from_agent(&engine, &id, "thread.access", json!({"id": id, "access": "full"}))), "thread.not_person");
    let id = full_thread(&engine);

    ok(&engine, "thread.send", json!({"id": id, "text": "Please RUNIT"}));
    wait_until("the agent to ask", || pending(&engine, &id).len() == 1);
    let ask = pending(&engine, &id).remove(0);
    assert_eq!(ask["tool"], "Bash");
    assert_eq!(ask["title"], "Run a command");
    assert!(ask["detail"].as_str().unwrap().starts_with("touch "), "{ask}");
    assert_eq!(ask["reason"], "Make a file");
    let thread = ok(&engine, "thread.get", json!({"id": id}))["thread"].clone();
    assert_eq!((thread["working"].clone(), thread["asking"].clone()), (json!(true), json!(1)), "the turn waits on the person");
    // The agent cannot answer its own ask, and nothing ran yet.
    assert_eq!(code(from_agent(&engine, &id, "thread.ask.answer", json!({"id": ask["id"], "allow": true}))), "thread.not_person");
    assert!(!dir.path().join("made").exists());

    let answered = ok(&engine, "thread.ask.answer", json!({"id": ask["id"], "allow": true}));
    assert_eq!((answered["status"].as_str(), answered["scope"].as_str()), (Some("allowed"), Some("once")));
    idle(&engine, &id);
    assert!(dir.path().join("made").exists(), "allowed, it ran");
    let told = responses(dir.path());
    assert_eq!(told[0]["response"]["response"]["behavior"], "allow");
    assert_eq!(told[0]["response"]["response"]["updatedInput"]["description"], "Make a file", "the agent gets its input back whole");
    assert_eq!(code(engine.dispatch(Request::new(Actor::User, "thread.ask.answer", json!({"id": ask["id"], "allow": false})), Door::InProcess)), "thread.ask_answered");

    // Once is once: the next command asks again, and a denial says why to the agent.
    std::fs::remove_file(dir.path().join("made")).unwrap();
    ok(&engine, "thread.send", json!({"id": id, "text": "RUNIT again"}));
    wait_until("the agent to ask again", || pending(&engine, &id).len() == 1);
    let ask = pending(&engine, &id).remove(0);
    ok(&engine, "thread.ask.answer", json!({"id": ask["id"], "allow": false, "message": "not on my machine"}));
    idle(&engine, &id);
    assert!(!dir.path().join("made").exists(), "denied, nothing ran");
    let denial = &responses(dir.path())[1]["response"]["response"];
    assert_eq!(denial["behavior"], "deny");
    assert!(denial["message"].as_str().unwrap().contains("not on my machine"), "{denial}");

    // For the thread: the next command runs without asking.
    ok(&engine, "thread.send", json!({"id": id, "text": "RUNIT a third time"}));
    wait_until("the agent to ask a third time", || pending(&engine, &id).len() == 1);
    let ask = pending(&engine, &id).remove(0);
    ok(&engine, "thread.ask.answer", json!({"id": ask["id"], "allow": true, "scope": "thread"}));
    idle(&engine, &id);
    std::fs::remove_file(dir.path().join("made")).unwrap();
    ok(&engine, "thread.send", json!({"id": id, "text": "RUNIT once more"}));
    idle(&engine, &id);
    assert!(dir.path().join("made").exists(), "allowed for the thread, it ran unasked");
    assert_eq!(responses(dir.path()).len(), 4);
    let asked = ok(&engine, "thread.ask.list", json!({"thread_id": id}))["asks"].as_array().unwrap().len();
    assert_eq!(asked, 3, "the fourth was never asked");

    // Back to its data: what was allowed for the thread is forgotten, and the agent restarts
    // without its tools.
    ok(&engine, "thread.access", json!({"id": id, "access": "data"}));
    ok(&engine, "thread.access", json!({"id": id, "access": "full"}));
    ok(&engine, "thread.send", json!({"id": id, "text": "RUNIT after all that"}));
    wait_until("the agent to ask after the access was taken back", || pending(&engine, &id).len() == 1);
    ok(&engine, "thread.stop", json!({"id": id}));
    let asks = ok(&engine, "thread.ask.list", json!({"thread_id": id}))["asks"].as_array().unwrap().clone();
    assert_eq!(asks.last().unwrap()["status"], "cancelled", "a stopped agent's ask goes with it");
    assert_eq!(ok(&engine, "thread.get", json!({"id": id}))["thread"]["asking"], 0);

    let args = std::fs::read_to_string(dir.path().join("args.log")).unwrap();
    let first = args.lines().next().unwrap();
    assert!(first.starts_with("OPS=<unset> "), "every Relay op is listed: {first}");
    assert!(first.contains("--permission-prompt-tool stdio") && first.contains("--permission-mode default"), "{first}");
    assert!(!first.contains("--allowedTools"), "{first}");
}

#[test]
fn a_full_access_agent_reads_relay_freely_and_changes_it_only_once_allowed() {
    if !python() {
        return;
    }
    let engine = engine();
    let dir = tempfile::tempdir().unwrap();
    with_fake(&engine, dir.path());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let server = runtime.block_on(relay_core::socket::SocketServer::start_in(engine.clone(), dir.path().join("run"))).unwrap();
    std::fs::write(dir.path().join("sock"), server.path.display().to_string()).unwrap();
    let id = full_thread(&engine);
    // A read runs unasked, Relay's answering ops are never the agent's, and a change waits.
    std::fs::write(dir.path().join("tools.json"), json!(["mcp__relay__thread_list", "mcp__relay__thread_ask_answer", "mcp__relay__thread_rename"]).to_string()).unwrap();
    let calls = json!([
        ["thread.list", {}],
        ["thread.rename", {"id": id, "title": "Renamed once"}],
        ["thread.rename", {"id": id, "title": "Renamed twice"}],
        ["thread.ask.answer", {"id": 1, "allow": true}],
        ["settings.get", {}],
    ]);
    std::fs::write(dir.path().join("calls.json"), calls.to_string()).unwrap();

    ok(&engine, "thread.send", json!({"id": id, "text": "CALLRELAY"}));
    wait_until("the agent to ask for the rename", || pending(&engine, &id).len() == 1);
    let ask = pending(&engine, &id).remove(0);
    assert_eq!((ask["tool"].as_str(), ask["op"].as_str(), ask["title"].as_str()), (Some("mcp__relay__thread_rename"), Some("thread.rename"), Some("Rename a thread")));
    let told = responses(dir.path());
    assert_eq!(told[0]["response"]["response"]["behavior"], "allow", "a read runs unasked");
    assert_eq!(told[1]["response"]["response"]["behavior"], "deny", "the person answers asks");
    ok(&engine, "thread.ask.answer", json!({"id": ask["id"], "allow": true}));
    wait_until("the agent's calls", || dir.path().join("answers.json").exists());
    idle(&engine, &id);

    let answers: Value = serde_json::from_str(&std::fs::read_to_string(dir.path().join("answers.json")).unwrap()).unwrap();
    assert_eq!(answers[0]["ok"], true, "{answers}");
    assert_eq!(answers[1]["ok"], true, "allowed once: {answers}");
    assert_eq!(answers[2]["error"]["code"], "thread.unapproved", "once is spent: {answers}");
    assert_eq!(answers[3]["error"]["code"], "thread.not_person", "{answers}");
    assert_eq!(answers[4]["ok"], true, "every read: {answers}");
    assert_eq!(ok(&engine, "thread.get", json!({"id": id}))["thread"]["title"], "Renamed once");

    // A thread with its data alone reaches its own ops and nothing else, as before.
    ok(&engine, "thread.access", json!({"id": id, "access": "data"}));
    std::fs::remove_file(dir.path().join("answers.json")).unwrap();
    std::fs::write(dir.path().join("tools.json"), "[]").unwrap();
    let calls = json!([["money.summary", {}], ["settings.get", {}], ["thread.rename", {"id": id, "title": "No"}]]);
    std::fs::write(dir.path().join("calls.json"), calls.to_string()).unwrap();
    ok(&engine, "thread.send", json!({"id": id, "text": "CALLRELAY"}));
    wait_until("the data agent's calls", || dir.path().join("answers.json").exists());
    let answers: Value = serde_json::from_str(&std::fs::read_to_string(dir.path().join("answers.json")).unwrap()).unwrap();
    assert_eq!(answers[0]["ok"], true, "{answers}");
    assert_eq!(answers[1]["error"]["code"], "actor.peer", "{answers}");
    assert_eq!(answers[2]["error"]["code"], "actor.peer", "{answers}");
    idle(&engine, &id);
    drop(server);
}

#[test]
fn full_access_waits_for_the_turn_and_needs_a_claude_model() {
    let engine = engine();
    let id = full_thread(&engine);
    // A model on this PC is not given a shell, either way round.
    assert_eq!(code(engine.dispatch(Request::new(Actor::User, "thread.set", json!({"id": id, "model": "qwen35-4b"})), Door::InProcess)), "thread.access_local");
    ok(&engine, "thread.access", json!({"id": id, "access": "data"}));
    ok(&engine, "thread.set", json!({"id": id, "model": "qwen35-4b"}));
    assert_eq!(code(engine.dispatch(Request::new(Actor::User, "thread.access", json!({"id": id, "access": "full"})), Door::InProcess)), "thread.access_local");
    assert_eq!(code(engine.dispatch(Request::new(Actor::User, "thread.ask.answer", json!({"id": 99, "allow": true})), Door::InProcess)), "thread.ask_not_found");
    let listed = ok(&engine, "thread.list", json!({}));
    assert_eq!(listed["threads"][0]["access"], "data");
    assert_eq!(listed["threads"][0]["asking"], 0);
}
