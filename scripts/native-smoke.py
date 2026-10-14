#!/usr/bin/env python3
"""Real native client + disposable engine + fake providers. No user store or paid CLI."""
import json
import os
from pathlib import Path
import socket
import subprocess
import struct
import sys
import tempfile
import time
import uuid

# Every check here is an assert statement, and some perform the action they check;
# under -O they would vanish and the run would report success without doing anything.
if sys.flags.optimize:
    raise SystemExit("Run without python -O or PYTHONOPTIMIZE: this script checks with assert.")

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / ".impeccable/review"
OUT.mkdir(parents=True, exist_ok=True)
ENGINE = ROOT / "target/debug/relay"
NATIVE = ROOT / "target/debug/relay-native"
# The in-app roadmap regressions (apps/relay-native/src/smoke.rs, RELAY_NATIVE_ROADMAP).
ROADMAP_PARTS = ("notes", "files", "lifecycle", "tools", "registry")


def git(repo, *args):
    subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True)


def check_gtk_log(contents):
    for message in ("Trying to measure GtkPaned", "reported min width -", "Unable to register the application"):
        assert message not in contents, contents


with tempfile.TemporaryDirectory(prefix="relay-native-smoke-") as temporary:
    base = Path(temporary)
    runtime = base / "runtime"
    runtime.mkdir(mode=0o700)
    env = dict(os.environ, XDG_DATA_HOME=str(base / "data"),
               XDG_CONFIG_HOME=str(base / "config"), XDG_CACHE_HOME=str(base / "cache"),
               XDG_RUNTIME_DIR=str(runtime), RELAY_INSTANCE="test", RELAY_BIN=str(ENGINE), NO_COLOR="1")
    for key in ("RELAY_SESSION", "RELAY_TOKEN", "RELAY_BRIEF"):
        env.pop(key, None)
    provider = base / "provider"
    provider.write_text('''#!/usr/bin/env python3
import os, sys
if "--version" in sys.argv:
    print("native-smoke 1.0"); sys.exit()
if "auth" in sys.argv or "login" in sys.argv:
    print('{"loggedIn":true}'); sys.exit()
assert "NO_COLOR" not in os.environ, "Daemon log settings leaked into the interactive PTY"
assert os.environ["TERM"] == "xterm-256color" and os.environ["COLORTERM"] == "truecolor"
print("\\033[1;36mRELAY NATIVE VERIFICATION\\033[0m", flush=True)
print("\\033[31mRed \\033[32mGreen \\033[34mBlue \\033[38;2;217;119;87mTruecolor\\033[0m", flush=True)
print("Disposable provider fixture, no model connected.", flush=True)
print("Session: " + os.environ.get("RELAY_SESSION", "fixture"), flush=True)
print("\\nEngine owns this PTY. VTE renders its output.", flush=True)
print("  [ok] isolated worktree\\n  [ok] ordered output\\n  [ok] waiting for input", flush=True)
# Fixed limits for the status bar and the sidebar's usage card: a test engine never reads the
# providers' own files. A Codex session's Claude report is refused, and that is fine.
import json as _json, subprocess as _subprocess, time as _time
_now = int(_time.time())
_subprocess.run([os.environ["RELAY_BIN"], "cmd", "usage.report", _json.dumps({"session": os.environ.get("RELAY_SESSION", ""),
    "provider": "claude", "payload": {"five_hour": {"used_pct": 12, "resets_at": _now + 13290, "resets_in": "3h 41m"},
    "seven_day": {"used_pct": 29, "resets_at": _now + 4 * 86400, "resets_in": "4d 0h"}}})], capture_output=True)
for line in sys.stdin:
    if line.strip()=="native-burst":
        for i in range(2048): print(f"B{i:06d} " + "x"*88)
        print("BURST-END-"+os.environ["RELAY_SESSION"],flush=True)
    elif line.startswith("native-request "):
        # Only an agent may ask for a guardrail exception: this session asks as itself.
        import json, subprocess
        _, kind, value, scope = line.split()
        payload = json.dumps({"session": os.environ["RELAY_SESSION"], "kind": kind, "value": value,
                              "reason": "The native smoke fixture needs this exception to go on.", "scope": scope})
        done = subprocess.run([os.environ["RELAY_BIN"], "cmd", "guardrail.request", payload], capture_output=True, text=True)
        print("request: " + (done.stdout.strip() or done.stderr.strip()), flush=True)
    else: print("echo: " + line.rstrip(), flush=True)
''')
    provider.chmod(0o755)
    log = (OUT / "engine.log").open("w")
    engine = subprocess.Popen([str(ENGINE), "--instance", "test", "serve"], env=env, stdout=log, stderr=log)
    connection = socket.socket(socket.AF_UNIX)
    path = runtime / "relay-v4/test.sock"
    try:
        deadline = time.monotonic() + 15
        while True:
            try:
                connection.connect(str(path))
                break
            except (FileNotFoundError, ConnectionRefusedError):
                if engine.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError("Fixture engine failed; see engine.log")
                time.sleep(0.05)
        stream = connection.makefile("rwb", buffering=0)

        def call(op, payload):
            stream.write((json.dumps(dict(v=1, id=str(uuid.uuid4()), actor="user", op=op, payload=payload)) + "\n").encode())
            response = json.loads(stream.readline())
            if not response["ok"]:
                raise RuntimeError(f"{op}: {response['error']}")
            return response["result"]

        workspace = base / "workspace"
        repo = workspace / "Native verification with a long repository name"
        repo.mkdir(parents=True)
        git(repo, "init", "-q", "-b", "main")
        git(repo, "config", "user.name", "Native fixture")
        git(repo, "config", "user.email", "fixture@relay.test")
        (repo / "README.md").write_text("# Native verification\n\nDisposable smoke-test project.\n")
        git(repo, "add", ".")
        git(repo, "commit", "-qm", "fixture")
        ws = call("workspace.create", {"path": str(workspace)})
        project = call("project.add", {"workspace_id": ws["id"], "path": str(repo)})
        for name in ("claude", "codex"):
            call("settings.set", {"path": f"providers.{name}.path", "value": str(provider)})
        for title in ("Native terminal lifecycle", "Mailbox and review contracts", "Code workspace"):
            task = call("task.create", {"project_id": project["id"], "title": title, "body": "Verification fixture.", "column": "ready"})
            builder = call("session.create", {"project_id": project["id"], "provider": "claude", "role": "builder"})
            reviewer = call("session.create", {"project_id": project["id"], "provider": "codex", "role": "reviewer", "pair_with": builder["name"]})
            call("task.dispatch", {"task_id": task["id"], "session": builder["name"], "start": False})
            for session in (builder, reviewer):
                call("session.spawn", {"session": session["name"]})
        call("mailbox.send", {"project_id": project["id"], "to": reviewer["name"],
                              "text": "Terminal changes are ready for review. Check input ordering and teardown.", "priority": True})
        call("notes.create", {"project_id": project["id"], "title": "Native rebuild",
                              "body": "GTK4, VTE and GtkSourceView. Keep approval authority separate from review."})
        call("guardrail.config.set", {"project_id": project["id"], "patch": {"protected_paths": ["secret/*"]}})
        try:
            call("guardrail.gate", {"session": builder["name"], "kind": "write", "path": "secret/key", "new_text": "fixture"})
            raise AssertionError("The fixture action should be held")
        except RuntimeError as error:
            assert "held" in str(error), error
        assert len(call("guardrail.holds.list", {"project_id": project["id"]})["holds"]) == 1
        # Money's pages read Tally's sample household from the disposable ledger.
        assert call("money.sample", {})["transactions"] > 0
        desktop_env = dict(os.environ, RELAY_NATIVE_SOCKET=str(path), RELAY_INSTANCE="test")
        for key in ("RELAY_SESSION", "RELAY_TOKEN", "RELAY_BRIEF"):
            desktop_env.pop(key, None)
        initial_names={s["name"] for s in call("session.list",{"project_id":project["id"]})["sessions"]}
        for name in initial_names:
            deadline = time.monotonic() + 5
            while True:
                output = call("session.scrollback", {"session": name})["text"]
                if "Truecolor" in output:
                    break
                assert time.monotonic() < deadline, output
                time.sleep(0.05)
            assert "\x1b[31mRed" in output and "\x1b[38;2;217;119;87mTruecolor" in output, output
        def run_roadmap(parts):
            for part in parts:
                with (OUT / f"roadmap-{part}.log").open("w") as native_log:
                    native = subprocess.Popen([str(NATIVE)], env=dict(desktop_env,
                        RELAY_NATIVE_SCREENSHOT=str(OUT / f"roadmap-{part}.png"),
                        RELAY_NATIVE_FIXTURE="1", RELAY_NATIVE_ROADMAP=part),
                        stdout=native_log, stderr=native_log)
                    try:
                        assert native.wait(timeout=90) == 0, part
                    finally:
                        if native.poll() is None:
                            native.terminate()
                            native.wait(timeout=5)
                assert f"ROADMAP_OK={part}" in (OUT / f"roadmap-{part}.log").read_text(), part
                check_gtk_log((OUT / f"roadmap-{part}.log").read_text())
            print("Roadmap native regressions passed: " + ",".join(parts))
        # RELAY_SMOKE_ROADMAP_ONLY=notes,files narrows a run to those roadmap parts. Without it
        # every part runs, after the page captures and their assertions: the parts move, rename
        # and trash files, remove a registry entry and launch sessions, which would break the
        # session counts and README check below. registry goes last for the same reason.
        if os.environ.get("RELAY_SMOKE_ROADMAP_ONLY"):
            run_roadmap(os.environ["RELAY_SMOKE_ROADMAP_ONLY"].split(","))
            raise SystemExit(0)
        measurements=[]
        captured=[]
        # The start screen's opening, held at points of its timeline (start.rs).
        START_FRAMES = {"start-frame-20": "0.2", "start-frame-45": "0.45", "start-frame-60": "0.6", "start-frame-80": "0.8"}
        # RELAY_SMOKE_PAGES=start,start-frame-45 captures only those pages and stops there, for a
        # quick look at one screen; the checks after the captures need every page.
        PAGES_ONLY = [p for p in os.environ.get("RELAY_SMOKE_PAGES", "").split(",") if p]
        for viewport, size, page in (("desktop", "1440,900", "agents"), ("compact", "1024,768", "agents"), ("launch-preview", "1024,768", "launch-preview"), ("palette", "1024,768", "palette"), ("layouts", "1024,768", "layouts"), ("usage", "1440,900", "usage"), ("board", "1440,900", "board"), ("board-compact", "1024,768", "board"), ("mailbox", "1024,768", "mailbox"), ("guardrails", "1024,768", "guardrails"), ("code", "1440,900", "code"), ("resources", "1440,900", "resources"), *((name,"1440,900",name) for name in ("notes","modules","settings","skills","plugins","project-skills","project-plugins","notifications","devices","launch")),
                                     *((name,"1440,900",name) for name in ("money-home","money-transactions","money-data","start")), ("money-plan","1024,768","money-plan"), ("money-entry","1024,768","money-entry"),
                                     ("agent-settings","1440,900","agent-settings"), ("agent-settings-compact","1024,768","agent-settings"),
                                     *((frame,"1440,900","start") for frame in START_FRAMES)):
            if PAGES_ONLY and viewport not in PAGES_ONLY:
                continue
            output = OUT / f"{viewport}.png"
            output.unlink(missing_ok=True)
            native_env = dict(desktop_env, RELAY_NATIVE_SCREENSHOT=str(output), RELAY_NATIVE_SIZE=size, RELAY_NATIVE_FIXTURE="1",
                              RELAY_NATIVE_PAGE=page, RELAY_NATIVE_SMOKE_SECONDS="8" if viewport in ("desktop","launch") else "5")
            if viewport in START_FRAMES:
                native_env["RELAY_NATIVE_START_T"] = START_FRAMES[viewport]
            with (OUT / f"{viewport}.log").open("w") as native_log:
                native=subprocess.Popen([str(NATIVE)], env=native_env, stdout=native_log, stderr=native_log)
                if viewport=="desktop":
                    def sample():
                        stat=Path(f"/proc/{native.pid}/stat").read_text().split(") ",1)[1].split()
                        return int(stat[11])+int(stat[12]), int(stat[21])*os.sysconf("SC_PAGE_SIZE")
                    time.sleep(3)
                    ticks, _=sample(); started=time.monotonic()
                    time.sleep(3)
                    end_ticks,rss=sample();elapsed=time.monotonic()-started
                    measurements.append(dict(cpu_percent=round((end_ticks-ticks)/os.sysconf("SC_CLK_TCK")/elapsed*100,2),
                                             rss_mib=round(rss/1024/1024,1), sample_seconds=round(elapsed,2)))
                try:
                    assert native.wait(timeout=30)==0
                finally:
                    if native.poll() is None:
                        native.terminate()
                        native.wait(timeout=5)
            assert output.is_file(), f"No screenshot at {output}"
            captured.append(output.name)
            expected_size = (1080, 760) if page == "notes" else tuple(map(int, size.split(",")))
            # The agent settings capture is the popover alone, sized by its content.
            if page != "agent-settings":
                assert struct.unpack(">II", output.read_bytes()[16:24]) == expected_size, viewport
            contents = (OUT / f"{viewport}.log").read_text()
            check_gtk_log(contents)
            assert "Shell layouts verified" in contents, contents
            # smoke.rs holds the capture for these checks; their own lines prove they ran on this page.
            if page == "agents":
                assert "In-app confirmation accept and cancel verified" in contents, contents
            if page == "settings":
                assert "Settings save verified across categories" in contents, contents
            if page == "start":
                assert "Start screen dismiss verified" in contents, contents
            saved_layout = call("settings.get", {"path": f"native.layout.current.{project['id']}"})["value"]
            assert saved_layout["agent_layout"] == "grid", saved_layout
            assert "stylesheet:" not in contents and "gtk_widget_add_css_class:" not in contents, contents
        if PAGES_ONLY:
            print(json.dumps({"screenshots": captured}))
            raise SystemExit(0)
        (OUT/"measurements.json").write_text(json.dumps(measurements,indent=2)+"\n")
        live = call("session.list", {"project_id": project["id"]})["sessions"]
        assert len(live) == 9 and all(s["state"] == "running" for s in live), live
        builders = [s for s in live if s["role"]=="builder"]
        reviewers = [s for s in live if s["role"]=="reviewer"]
        assert len(builders)==5 and len(reviewers)==4, live
        launched=[s for s in live if s["name"] not in initial_names]
        assert len(launched)==3 and len({s["worktree"] for s in launched})==1, launched
        assert any(s["task_id"] is not None for s in launched), launched
        assert any(t["title"]=="Native task edit verified" for t in call("task.list",{"project_id":project["id"]})["tasks"])
        assert any(n["body"]=="Native note save verified." for n in call("notes.list",{"project_id":project["id"]})["notes"])
        for _ in range(2):
            extra=call("session.create",{"project_id":project["id"],"provider":"claude","role":"builder"})
            call("session.spawn",{"session":extra["name"]})
        burst_env=dict(desktop_env,RELAY_NATIVE_SCREENSHOT=str(OUT/"burst.png"),RELAY_NATIVE_SIZE="1440,900",RELAY_NATIVE_FIXTURE="1",RELAY_NATIVE_PAGE="agents",RELAY_NATIVE_BURST="1",RELAY_NATIVE_SMOKE_SECONDS="7")
        with (OUT/"burst.log").open("w") as burst_log:
            burst=subprocess.Popen([str(NATIVE)],env=burst_env,stdout=burst_log,stderr=burst_log)
            try: assert burst.wait(timeout=20)==0
            finally:
                if burst.poll() is None: burst.terminate();burst.wait(timeout=5)
        assert "BURST_RENDERED=11" in (OUT/"burst.log").read_text()
        live=call("session.list",{"project_id":project["id"]})["sessions"]
        for session in live:
            output=call("session.scrollback", {"session": session["name"]})["text"]
            if session["name"] in initial_names:
                assert f"echo: native-paste-check-{session['name']}" in output, output
            call("session.input", {"session": session["name"], "data": "survived-window-close\n"})
            call("session.park", {"session": session["name"]})
        saved=call("file.read", {"project_id":project["id"],"path":"README.md"})["text"]
        assert "Native editor save verified." in saved, saved
        run_roadmap(ROADMAP_PARTS)
        print(json.dumps({"screenshots": captured + ["burst.png"], "sessions_survived_window_close": len(live), "native_paste_echoes_verified":6,"native_editor_saved":True,"native_task_saved":True,"native_note_saved":True,"native_review_group_launched":True,"burst_native_tail_markers":11,"burst_lines_per_session":2048,"burst_completion_budget_seconds":5,"roadmap_parts_passed": list(ROADMAP_PARTS),"real_provider_calls": 0}))
    finally:
        connection.close()
        engine.terminate()
        try:
            engine.wait(timeout=10)
        except subprocess.TimeoutExpired:
            engine.kill()
            engine.wait()
        log.close()
