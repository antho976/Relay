#!/usr/bin/env python3
"""The Threads space against a disposable engine: a fake Claude Code that speaks stream-json
answers a thread's messages (a tool call, its answer, a reply in Markdown), then relay-native
captures the conversation and a new thread.

    python3 scripts/threads-smoke.py [shot ...]

Shots (default the first three): thread, threads, thread-compact, thread-tall (1440x1700),
thread-working (a thread whose agent is mid-turn), thread-edit (the newest message's Edit key
pressed: its words back in the box). Needs target/debug/relay and
target/debug/relay-native, and a display: run it inside a headless compositor
(kwin_wayland --virtual), never on someone's desktop. Captures and logs go to
.impeccable/review/threads-*. No model is called and nothing reaches the person's own engine."""
import json, os, socket, subprocess, sys, tempfile, time, uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / ".impeccable/review"; OUT.mkdir(parents=True, exist_ok=True)
SHOTS = sys.argv[1:] or ["thread", "threads", "thread-compact"]
# thread-working opens a thread whose agent is mid-turn; it runs last, as it becomes the newest.
SHOTS.sort(key=lambda shot: shot == "thread-working")
ENGINE = ROOT / "target/debug/relay"
NATIVE = ROOT / "target/debug/relay-native"

# Claude Code in print mode with stream-json in and out, as threads.rs runs it: an init line,
# then for each message a tool call, its answer, the reply and the turn's result.
FAKE_CLAUDE = r'''#!/usr/bin/env python3
import json, sys, time, uuid
if "--version" in sys.argv:
    print("2.1.0 (Claude Code)"); sys.exit()
if "auth" in sys.argv or "login" in sys.argv:
    print('{"loggedIn":true}'); sys.exit()
def out(o):
    print(json.dumps(o), flush=True)
out({"type": "system", "subtype": "init", "session_id": str(uuid.uuid4()),
     "mcp_servers": [{"name": "relay", "status": "connected"}], "tools": ["mcp__relay__money_summary", "mcp__relay__money_series"]})
REPLIES = [
    ("mcp__relay__money_summary", "You have **$299 left** for October, with 15 days to go. Dining and shopping are past their budgets; "
     "groceries and transport are on pace.\n\n| Budget | Spent | Left |\n|---|---:|---:|\n| Dining | $383 | -$63 |\n"
     "| Shopping | $158 | -$8 |\n| Groceries | $457 | $43 |\n| Transport | $102 | $18 |\n\n"
     "At $20 a day you stay inside the month. Want me to move $60 from groceries to dining?"),
    ("mcp__relay__money_series", "Week by week, dining went **$71, $96, $142, $74**: the third week had two dinners out "
     "and a birthday. Without them you would be $18 under.\n\n- Café Olimpico: 6 visits, $112\n- Pho Lien: 3 visits, $79\n\n"
     "Moving $60 keeps every budget positive."),
    ("mcp__relay__money_tx_list", "### What I would change\n\n1. Move **$60** from *Groceries* to *Dining*.\n2. Set a `$25` cap on "
     "weeknight dinners.\n3. Look again on the 25th.\n\nThe rule, as Tally keeps it:\n\n```json\n{\"budget\": \"Dining\", "
     "\"cap\": 2500, \"days\": [\"Mon\", \"Tue\", \"Wed\", \"Thu\"]}\n```\n\n> Budgets move with Undo for a day.\n\n"
     "See [Tally's budget guide](https://example.com/tally/budgets) for more."),
]
turn = 0
for line in sys.stdin:
    try:
        msg = json.loads(line)
    except ValueError:
        continue
    if msg.get("type") != "user":
        continue
    text = json.dumps(msg)
    if "take your time" in text:
        # Mid-turn for the working shot: a tool called, its answer not yet in.
        out({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": "slow", "name": "mcp__relay__money_series", "input": {}}]}})
        time.sleep(60)
        continue
    tool, reply = REPLIES[min(turn, len(REPLIES) - 1)]
    call = "t%d" % turn
    out({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": call, "name": tool, "input": {}}]}})
    out({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": call, "content": '{"ok": true}'}]}})
    out({"type": "assistant", "message": {"content": [{"type": "text", "text": reply}]}})
    out({"type": "result", "subtype": "success", "is_error": False, "result": reply})
    turn += 1
'''


with tempfile.TemporaryDirectory(prefix="relay-threads-shot-") as temporary:
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
    log = (OUT / "threads-engine.log").open("w")
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
                    raise RuntimeError("engine failed; see threads-engine.log")
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

        def answered(tid, replies):
            """Wait for the thread's agent to have said `replies` things and finished its turn."""
            deadline = time.monotonic() + 30
            while time.monotonic() < deadline:
                got = call("thread.get", {"id": tid})
                said = [m for m in got["messages"] if m["role"] == "assistant" and any(b.get("type") == "text" for b in m["body"].get("blocks", []))]
                errors = [m for m in got["messages"] if m["role"] == "error"]
                assert not errors, errors
                if len(said) >= replies and not got["thread"].get("working"):
                    return got
                time.sleep(0.1)
            raise RuntimeError(f"thread {tid} did not answer; see threads-engine.log")

        ws_dir = base / "workspace"; repo = ws_dir / "Tally"; repo.mkdir(parents=True)
        subprocess.run(["git", "-C", str(repo), "init", "-q", "-b", "main"], check=True)
        call("workspace.create", {"path": str(ws_dir)})
        call("settings.set", {"path": "providers.claude.path", "value": str(claude)})
        thread = call("thread.create", {"text": "Am I on pace this month?"})
        tid = thread["id"]
        answered(tid, 1)
        call("thread.send", {"id": tid, "text": "Why is dining so far over?"})
        answered(tid, 2)
        call("thread.send", {"id": tid, "text": "What would you change?"})
        got = answered(tid, 3)
        tools = [m for m in got["messages"] if m["role"] == "tool"]
        assert len(tools) == 3 and not any(t["body"]["is_error"] for t in tools), tools

        native_env = dict(os.environ, RELAY_NATIVE_SOCKET=str(path), RELAY_INSTANCE="test", XDG_CACHE_HOME=str(base / "native-cache"))
        for key in ("RELAY_SESSION", "RELAY_TOKEN", "RELAY_BRIEF"):
            native_env.pop(key, None)
        for shot in SHOTS:
            if shot == "thread-working":
                # A second thread, its agent mid-turn: the newest, so this shot opens it.
                call("thread.create", {"text": "Plan next month, take your time"})
                time.sleep(1.0)
            output = OUT / f"threads-{shot}.png"; output.unlink(missing_ok=True)
            # thread-tall shows a whole conversation at once (the compositor must be as tall).
            size = "1024,768" if shot.endswith("compact") else "1440,1700" if shot.endswith("tall") else "1440,900"
            page = "threads" if shot == "threads" else "thread"
            e = dict(native_env, RELAY_NATIVE_SCREENSHOT=str(output), RELAY_NATIVE_SIZE=size, RELAY_NATIVE_FIXTURE="1",
                     RELAY_NATIVE_PAGE=page, RELAY_NATIVE_SMOKE_SECONDS="6")
            if shot == "thread-edit":
                e["RELAY_SMOKE_THREAD_EDIT"] = "1"
            with (OUT / f"threads-{shot}.log").open("w") as nl:
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
