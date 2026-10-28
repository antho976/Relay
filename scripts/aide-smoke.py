#!/usr/bin/env python3
"""The Aide space against a disposable engine: two bots, one of which has run and spoken, and a
Chief of Staff asked for a new bot, all answered by a fake Claude Code that speaks stream-json and
calls Aide's ops through the engine's door as a real run's agent would. Then relay-native captures
the space.

    python3 scripts/aide-smoke.py [shot ...]

Shots (default all): aide (the Chief of Staff's chat: a bot's report and a proposed bot),
aide-bot (the first bot's chat: Relay's words that woke it, what it said, the person's question
and its answer), aide-start (the start screen with its three cards). Needs target/debug/relay
and target/debug/relay-native, and a display: run it inside a headless compositor
(kwin_wayland --virtual), never on someone's desktop. Captures and logs go to
.impeccable/review/aide-*. No model is called and nothing reaches the person's own engine."""
import json, os, socket, subprocess, sys, tempfile, time, uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / ".impeccable/review"; OUT.mkdir(parents=True, exist_ok=True)
SHOTS = sys.argv[1:] or ["aide", "aide-bot", "aide-start"]
ENGINE = ROOT / "target/debug/relay"
NATIVE = ROOT / "target/debug/relay-native"

# Claude Code in print mode, as threads.rs runs it for an Aide thread. A run's wake is answered by
# saying something through aide.notify and ending the run; the person asking the Chief of Staff
# for a bot is answered by a proposal; any other message by a short reply. The ops go through
# `relay q`, inside this agent's process tree, so the door knows the call is Aide's.
FAKE_CLAUDE = r'''#!/usr/bin/env python3
import json, os, subprocess, sys, uuid
if "--version" in sys.argv:
    print("2.1.0 (Claude Code)"); sys.exit()
if "auth" in sys.argv or "login" in sys.argv:
    print('{"loggedIn":true}'); sys.exit()
def out(o):
    print(json.dumps(o), flush=True)
def op(name, payload):
    subprocess.run([os.environ["RELAY_BIN"], "--instance", "test", "q", name, json.dumps(payload)], capture_output=True, timeout=20)
def tool(name, payload, said):
    call = "t" + uuid.uuid4().hex[:8]
    out({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": call, "name": "mcp__relay__" + name.replace(".", "_"), "input": payload}]}})
    op(name, payload)
    out({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": call, "content": '{"ok": true}'}]}})
def reply(text):
    out({"type": "assistant", "message": {"content": [{"type": "text", "text": text}]}})
    out({"type": "result", "subtype": "success", "is_error": False, "result": text})
out({"type": "system", "subtype": "init", "session_id": str(uuid.uuid4()), "mcp_servers": [{"name": "relay", "status": "connected"}], "tools": ["mcp__relay__aide_notify", "mcp__relay__session_list"]})
for line in sys.stdin:
    try:
        msg = json.loads(line)
    except ValueError:
        continue
    if msg.get("type") != "user":
        continue
    text = json.dumps(msg)
    if "[Aide" in text:
        tool("session.list", {}, "")
        tool("aide.notify", {"title": "Two agents are waiting on you", "level": "interrupt",
             "body": "**amber-walrus** asked which avatar style to use (12 min ago), and **coral-marten** wants a review of the compaction fix. Both are on RelayV4."}, "")
        tool("aide.run.finish", {"outcome": "done", "note": "Two waiting"}, "")
        reply("Told you about the two agents waiting.")
    elif "bot" in text.lower() and "blocked" in text.lower():
        tool("aide.job.propose", {"spec": {"name": "Blocked agent", "instructions": "When an agent has waited on me for 30 minutes, tell me which one and what it asked.",
             "rules": [{"kind": "stuck", "minutes": 30}], "level": "interrupt"}, "why": "You asked for a bot that watches for blocked agents."}, "")
        reply("Drafted **Blocked agent**: it wakes when an agent has waited on you for 30 minutes and tells you which one. It waits for you to turn it on.")
    else:
        reply("amber-walrus is waiting on the avatar question: initials in a circle, or the provider's mark. coral-marten's fix is in review; nothing else is stuck.")
'''


def wait_for(what, check, seconds=30):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        got = check()
        if got:
            return got
        time.sleep(0.1)
    raise RuntimeError(f"{what} did not happen; see aide-engine.log")


with tempfile.TemporaryDirectory(prefix="relay-aide-shot-") as temporary:
    base = Path(temporary)
    runtime = base / "runtime"; runtime.mkdir(mode=0o700)
    env = dict(os.environ, XDG_DATA_HOME=str(base / "data"), XDG_CONFIG_HOME=str(base / "config"),
               XDG_CACHE_HOME=str(base / "cache"), XDG_RUNTIME_DIR=str(runtime), RELAY_INSTANCE="test",
               RELAY_BIN=str(ENGINE), NO_COLOR="1")
    for key in ("RELAY_SESSION", "RELAY_TOKEN", "RELAY_BRIEF", "WAYLAND_DISPLAY"):
        env.pop(key, None)
    claude = base / "claude"
    claude.write_text(FAKE_CLAUDE)
    claude.chmod(0o755)
    log = (OUT / "aide-engine.log").open("w")
    engine = subprocess.Popen([str(ENGINE), "--instance", "test", "serve"], env=env, stdout=log, stderr=log)
    path = runtime / "relay-v4/test.sock"
    conn = socket.socket(socket.AF_UNIX)
    try:
        deadline = time.monotonic() + 20
        while True:
            try:
                conn.connect(str(path)); break
            except (FileNotFoundError, ConnectionRefusedError):
                if engine.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError("engine failed; see aide-engine.log")
                time.sleep(0.05)
        stream = conn.makefile("rwb")

        def call(op, payload):
            stream.write((json.dumps(dict(v=1, id=str(uuid.uuid4()), actor="user", op=op, payload=payload)) + "\n").encode())
            stream.flush()
            while True:
                r = json.loads(stream.readline())
                if "ok" in r:
                    break
            if not r["ok"]:
                raise RuntimeError(f"{op}: {r['error']}")
            return r["result"]

        def messages(thread):
            return call("thread.get", {"id": thread})["messages"]

        def settled(thread, replies):
            def done():
                got = call("thread.get", {"id": thread})
                said = [m for m in got["messages"] if m["role"] == "assistant" and any(b.get("type") == "text" for b in m["body"].get("blocks", []))]
                errors = [m for m in got["messages"] if m["role"] == "error"]
                assert not errors, errors
                return len(said) >= replies and not got["thread"].get("working")
            wait_for(f"thread {thread}'s reply", done)

        call("settings.set", {"path": "providers.claude.path", "value": str(claude)})
        watch = call("aide.job.save", {"spec": {"name": "Agents waiting", "instructions": "Tell me which agents are waiting on me, and what they asked.",
                                                "sources": ["dev"], "schedule": {"every": "weekdays", "hour": 9}, "level": "interrupt"}})
        call("aide.job.save", {"spec": {"name": "Morning brief", "instructions": "Every morning: what the agents did overnight, Tally's pace, Arbiter's day.",
                                        "sources": ["dev", "money"], "schedule": {"every": "day", "hour": 7}}})
        chief = call("aide.chat", {})["thread_id"]
        run = call("aide.run.now", {"job_id": watch["id"]})
        wait_for("the run to end", lambda: call("aide.run.get", {"id": run["id"]})["state"] not in ("queued", "running"))
        bot = call("aide.chat", {"job_id": watch["id"]})["thread_id"]
        wait_for("the bot's note", lambda: any(m["role"] == "note" for m in messages(bot)))
        call("thread.send", {"id": bot, "text": "Which one should I answer first?"})
        settled(bot, 2)
        call("thread.send", {"id": chief, "text": "Make a bot that tells me when an agent is blocked for more than 30 minutes"})
        settled(chief, 1)
        wait_for("the proposal's note", lambda: any(m["role"] == "note" and m["body"].get("proposal") for m in messages(chief)))

        native_env = dict(os.environ, RELAY_NATIVE_SOCKET=str(path), RELAY_INSTANCE="test", XDG_CACHE_HOME=str(base / "native-cache"))
        for key in ("RELAY_SESSION", "RELAY_TOKEN", "RELAY_BRIEF"):
            native_env.pop(key, None)
        for shot in SHOTS:
            output = OUT / f"{shot}.png"; output.unlink(missing_ok=True)
            page = "start" if shot == "aide-start" else shot
            e = dict(native_env, RELAY_NATIVE_SCREENSHOT=str(output), RELAY_NATIVE_SIZE="1440,900", RELAY_NATIVE_FIXTURE="1",
                     RELAY_NATIVE_PAGE=page, RELAY_NATIVE_SMOKE_SECONDS="6")
            if shot == "aide-start":
                e["RELAY_NATIVE_START_T"] = "1"
            with (OUT / f"{shot}.log").open("w") as nl:
                native = subprocess.Popen([str(NATIVE)], env=e, stdout=nl, stderr=nl)
                try:
                    native.wait(timeout=60)
                finally:
                    if native.poll() is None:
                        native.terminate(); native.wait(timeout=5)
            print(shot, "->", output, output.is_file())
    finally:
        engine.terminate()
        engine.wait(timeout=10)
