#!/usr/bin/env python3
"""The Dev space's assistant against a disposable engine: a fake Claude Code that speaks
stream-json answers the assistant's messages (a tool call, its answer, a reply with task,
session and note cards, or an `open` block), then relay-native captures the drawer.

    python3 scripts/assistant-smoke.py [shot ...]

Shots (default all): cards (the drawer over the board, a reply with live cards), settings (the
gear's "without asking" switches), history (earlier chats), model (the picker: Claude's models,
the models on this PC, efforts), open (a new chat's "Open the board"
suggestion pressed from the mail page: the reply's `open` block takes the window to the board).
Needs target/debug/relay and target/debug/relay-native, and a display: run it inside a headless
compositor (kwin_wayland --virtual), never on someone's desktop. Captures and logs go to
.impeccable/review/assistant-*. No model is called and nothing reaches the person's own engine."""
import json, os, socket, subprocess, sys, tempfile, time, uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / ".impeccable/review"; OUT.mkdir(parents=True, exist_ok=True)
SHOTS = sys.argv[1:] or ["cards", "settings", "history", "model", "open"]
ENGINE = ROOT / "target/debug/relay"
NATIVE = ROOT / "target/debug/relay-native"

# Claude Code in print mode with stream-json in and out, as threads.rs runs the assistant: an
# init line, then for each message a tool call, its answer, the reply and the turn's result.
# The reply depends on the words: SESSION is the fixture session's name.
FAKE_CLAUDE = r'''#!/usr/bin/env python3
import json, sys, uuid
if "--version" in sys.argv:
    print("2.1.0 (Claude Code)"); sys.exit()
if "auth" in sys.argv or "login" in sys.argv:
    print('{"loggedIn":true}'); sys.exit()
def out(o):
    print(json.dumps(o), flush=True)
out({"type": "system", "subtype": "init", "session_id": str(uuid.uuid4()),
     "mcp_servers": [{"name": "relay", "status": "connected"}], "tools": ["mcp__relay__task_list", "mcp__relay__dashboard_get"]})
for n, line in enumerate(sys.stdin):
    try:
        msg = json.loads(line)
    except ValueError:
        continue
    if msg.get("type") != "user":
        continue
    text = json.dumps(msg).lower()
    if "open the board" in text:
        tool, given, reply = "mcp__relay__task_list", {"project_id": 1, "column": "ready"}, \
            "Opening the board. One task is ready to pick up:\n\n```open\nboard\n```\n\n```task\n1\n```"
    else:
        tool, given, reply = "mcp__relay__task_list", {"project_id": 1}, (
            "Yes: **#1** covers it, and it is ready to pick up.\n\n```task\n1\n```\n\n"
            "SESSION is set up for it but has not started, and the release note lists the fix:\n\n"
            "```session\nSESSION\n```\n\n```note\nNOTE\n```\n\nWant me to launch SESSION on it?")
    call = "t%d" % n
    out({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": call, "name": tool, "input": given}]}})
    out({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": call, "content": '{"tasks": []}'}]}})
    out({"type": "assistant", "message": {"content": [{"type": "text", "text": reply}]}})
    out({"type": "result", "subtype": "success", "is_error": False, "result": reply})
'''


with tempfile.TemporaryDirectory(prefix="relay-assistant-shot-") as temporary:
    base = Path(temporary)
    runtime = base / "runtime"; runtime.mkdir(mode=0o700)
    env = dict(os.environ, XDG_DATA_HOME=str(base / "data"), XDG_CONFIG_HOME=str(base / "config"),
               XDG_CACHE_HOME=str(base / "cache"), XDG_RUNTIME_DIR=str(runtime), RELAY_INSTANCE="test",
               RELAY_BIN=str(ENGINE), NO_COLOR="1")
    for key in ("RELAY_SESSION", "RELAY_TOKEN", "RELAY_BRIEF", "WAYLAND_DISPLAY"):
        env.pop(key, None)
    log = (OUT / "assistant-engine.log").open("w")
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
                    raise RuntimeError("engine failed; see assistant-engine.log")
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
            raise RuntimeError(f"thread {tid} did not answer; see assistant-engine.log")

        ws_dir = base / "workspace"; repo = ws_dir / "Shooter"; repo.mkdir(parents=True)
        subprocess.run(["git", "-C", str(repo), "init", "-q", "-b", "main"], check=True)
        subprocess.run(["git", "-C", str(repo), "-c", "user.name=Relay", "-c", "user.email=relay@example.com",
                        "commit", "-q", "--allow-empty", "-m", "Start"], check=True)
        workspace = call("workspace.create", {"path": str(ws_dir)})
        project = call("project.add", {"workspace_id": workspace["id"], "path": str(repo)})
        task = call("task.create", {"project_id": project["id"], "title": "Fix the login bug: the password field loses focus after a failed sign-in",
                                    "column": "ready", "priority": "high"})
        call("task.create", {"project_id": project["id"], "title": "Ship the onboarding checklist", "column": "backlog"})
        note = call("notes.create", {"project_id": project["id"], "title": "Release checklist",
                                     "body": "- Login focus fix\n- Onboarding copy pass\n- Bump the version"})
        session = call("session.create", {"project_id": project["id"], "provider": "claude", "role": "builder", "task_id": task["id"]})
        claude = base / "claude"
        claude.write_text(FAKE_CLAUDE.replace("SESSION", session["name"]).replace("NOTE", str(note["id"])))
        claude.chmod(0o755)
        call("settings.set", {"path": "providers.claude.path", "value": str(claude)})
        older = call("thread.create", {"kind": "assistant", "text": "What are my sessions doing right now?"})
        answered(older["id"], 1)
        thread = call("thread.create", {"kind": "assistant", "text": "Is there a task for the login bug?"})
        answered(thread["id"], 1)

        native_env = dict(os.environ, RELAY_NATIVE_SOCKET=str(path), RELAY_INSTANCE="test", XDG_CACHE_HOME=str(base / "native-cache"))
        for key in ("RELAY_SESSION", "RELAY_TOKEN", "RELAY_BRIEF"):
            native_env.pop(key, None)
        clicks = {
            "cards": ("board", "assistant-key"),
            "settings": ("board", "assistant-key,assistant-settings"),
            "history": ("board", "assistant-key,assistant-history"),
            "model": ("board", "assistant-key,assistant-model"),
            "open": ("mailbox", "assistant-key,assistant-new,assistant-suggestion-2"),
        }
        for shot in SHOTS:
            page, click = clicks[shot]
            output = OUT / f"assistant-{shot}.png"; output.unlink(missing_ok=True)
            e = dict(native_env, RELAY_NATIVE_SCREENSHOT=str(output), RELAY_NATIVE_SIZE="1440,900", RELAY_NATIVE_FIXTURE="1",
                     RELAY_NATIVE_PAGE=page, RELAY_NATIVE_CLICK=click, RELAY_NATIVE_SMOKE_SECONDS="8")
            if shot in ("settings", "history", "model"):
                # A menu is a surface of its own: captured alone.
                e["RELAY_NATIVE_CAPTURE_POPOVER"] = "1"
            with (OUT / f"assistant-{shot}.log").open("w") as nl:
                native = subprocess.Popen([str(NATIVE)], env=e, stdout=nl, stderr=nl)
                try:
                    native.wait(timeout=60)
                finally:
                    if native.poll() is None:
                        native.terminate(); native.wait(timeout=5)
            print(shot, "->", output, output.is_file())
        if "open" in SHOTS:
            # The suggestion was sent from the drawer: its reply is the newest chat's.
            newest = call("thread.list", {"kind": "assistant"})["threads"][0]
            got = answered(newest["id"], 1)
            assert newest["title"] == "Open the board", newest
            print("open: the reply holds", sum(1 for m in got["messages"] if m["role"] == "assistant"), "assistant messages")
    finally:
        engine.terminate()
        engine.wait(timeout=10)
