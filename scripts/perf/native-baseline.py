#!/usr/bin/env python3
"""The GTK client's half of the baseline. Runs only on a machine that can build relay-native.

For each page the shell has, it opens the real window against a disposable engine with fake
providers, lets it settle, and samples the client process: CPU during startup, CPU and wakeups
while sitting on the page, RSS, thread count. With `perf` installed it also records a sampling
profile per page and folds it by shared object and by symbol, which is the "where did the CPU
go" half. With `GDK_DEBUG=frames` GTK's own frame log is kept beside the numbers.

    python3 scripts/perf/native-baseline.py [--out-dir docs/perf/runs/<date>/native] [--seconds 8]

It never touches the user's store, never launches a paid model, and leaves screenshots under the
run directory. Written blind on a machine without GTK 4.22: if a page name or environment
variable has drifted, the log for that page says so and the others still run.
"""
import argparse
import datetime
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import uuid
from pathlib import Path

# The waits here end on assert statements; under -O a stalled wait would never end.
if sys.flags.optimize:
    raise SystemExit("Run without python -O or PYTHONOPTIMIZE: this script times out with assert.")

ROOT = Path(__file__).resolve().parents[2]
ENGINE = ROOT / "target/debug/relay"
NATIVE = ROOT / "target/debug/relay-native"
CLK_TCK = os.sysconf("SC_CLK_TCK")
PAGE = os.sysconf("SC_PAGE_SIZE")
# Seconds the client stays open past the 2 s settle and the measured window, so the window
# never overlaps its scheduled capture and exit (perf record returns a little late).
EXIT_MARGIN = 5

PAGES = ["agents", "board", "code", "notes", "modules", "settings", "skills", "dashboard", "notifications", "devices", "launch-preview", "palette", "layouts"]

PROVIDER = '''#!/usr/bin/env python3
import os, sys, time
if "--version" in sys.argv:
    print("native-perf 1.0"); sys.exit()
if "auth" in sys.argv or "login" in sys.argv:
    print('{"loggedIn":true}'); sys.exit()
print("\\033[1;36mRELAY NATIVE BASELINE\\033[0m", flush=True)
print("Disposable provider fixture, no model connected.", flush=True)
for line in sys.stdin:
    line = line.rstrip("\\n")
    if line == "native-burst":
        for i in range(2048): print(f"B{i:06d} " + "x" * 88)
        print("BURST-END-" + os.environ.get("RELAY_SESSION", ""), flush=True)
    elif line.startswith("tick "):
        n, ms = line.split()[1:3]
        for i in range(int(n)):
            print(f"tick {i} " + "x" * 60, flush=True); time.sleep(int(ms) / 1000)
    else:
        print("echo: " + line, flush=True)
'''


def log(msg):
    print(f"[native-baseline] {msg}", file=sys.stderr, flush=True)


def counters(pid):
    stat = Path(f"/proc/{pid}/stat").read_text().split(") ", 1)[1].split()
    # Per thread: a thread that exits inside a window takes its switches with it, so a plain sum
    # went negative when the client's worker threads wound down (2026-10-14). `delta` counts only
    # threads alive at both ends.
    vol = {}
    for task in Path(f"/proc/{pid}/task").iterdir():
        try:
            for line in (task / "status").read_text().splitlines():
                if line.startswith("voluntary_ctxt_switches"):
                    vol[task.name] = int(line.split()[1])
        except OSError:
            pass
    return {"cpu_ticks": int(stat[11]) + int(stat[12]), "rss": int(stat[21]) * PAGE, "vol": vol, "threads": len(vol)}


def window(pid, seconds):
    a = counters(pid)
    t0 = time.monotonic()
    time.sleep(seconds)
    b = counters(pid)
    return delta(a, b, time.monotonic() - t0)


def delta(a, b, el):
    return {"seconds": round(el, 2), "cpu_percent": round((b["cpu_ticks"] - a["cpu_ticks"]) / CLK_TCK / el * 100, 2),
            "cpu_ms": round((b["cpu_ticks"] - a["cpu_ticks"]) / CLK_TCK * 1000, 1), "wakeups_per_s": round(sum(b["vol"][t] - a["vol"][t] for t in a["vol"].keys() & b["vol"].keys()) / el, 1),
            "rss_mb": round(b["rss"] / 1024 / 1024, 1), "threads": b["threads"]}


def perf_record(pid, seconds, out_data):
    """A sampling profile of the client for `seconds`, with the counter window over the same seconds.

    The counters are read on both sides of `perf record` and nothing else, so the steady numbers
    describe the profiled window; folding the report (perf_fold) waits until the client is gone."""
    a = counters(pid)
    t0 = time.monotonic()
    rec = subprocess.run(["perf", "record", "-F", "997", "-g", "-p", str(pid), "-o", str(out_data), "--", "sleep", str(seconds)], capture_output=True, text=True)
    steady = delta(a, counters(pid), time.monotonic() - t0)
    return steady, (None if rec.returncode == 0 else {"error": rec.stderr[-500:]})


def perf_fold(out_data):
    """Fold a recorded profile by DSO and by symbol."""
    result = {}
    for key, sort in (("by_dso", "dso"), ("by_symbol", "dso,symbol")):
        rep = subprocess.run(["perf", "report", "-i", str(out_data), "--stdio", "--no-children", "--percent-limit", "0.5", "--sort", sort], capture_output=True, text=True)
        rows = []
        for line in rep.stdout.splitlines():
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            parts = line.split(None, 2 if sort == "dso" else 3)
            if parts and parts[0].endswith("%"):
                try:
                    rows.append({"pct": float(parts[0][:-1]), "what": " ".join(parts[1:])[:140]})
                except ValueError:
                    pass
        result[key] = rows[:40]
    return result


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out-dir", type=Path, default=ROOT / "docs/perf/runs" / datetime.date.today().isoformat() / "native")
    ap.add_argument("--seconds", type=int, default=8, help="how long each page sits open after the 2 s settle")
    ap.add_argument("--size", default="1440,900")
    ap.add_argument("--pages", default=",".join(PAGES))
    # GTK 4's text cursor fades at the frame rate until `gtk-cursor-blink-timeout` (10 s) after the
    # last input; a page that focuses an entry read as 30-126 wakeups/s "at rest" in an 8 s window
    # that started at 2 s (2026-10-14). The desktop's settings portal overrides a settings.ini, so
    # the window waits the blink out instead.
    ap.add_argument("--quiet-after", type=float, default=10.5, help="seconds to wait after the startup sample before the at-rest window (cursor blink)")
    ap.add_argument("--hidden-seconds", type=int, default=65,
                    help="how long the agents page sits minimized, and on screen, for the hidden-window rows (lead 11)")
    ap.add_argument("--hidden-only", action="store_true", help="only the hidden-window rows")
    ap.add_argument("--calls-page", default="",
                    help="count the requests the client sends from launch to 10 s on this page, three launches (e.g. code), and nothing else")
    ap.add_argument("--connect", type=int, default=0,
                    help="time N bus.ping on the shared connection and N on a connection of their own (lead 13), and nothing else")
    ap.add_argument("--board-tasks", default="",
                    help="comma-separated board sizes (e.g. 400,2000): time the board's opening at each (lead 8), and nothing else")
    ap.add_argument("--board-profile", action="store_true",
                    help="with --board-tasks: the first launch of each size runs under perf record from just before the timed opening")
    ap.add_argument("--burst-only", action="store_true",
                    help="only the session-event row: user-space instructions on the GTK thread per session.changed on the Agents page")
    ap.add_argument("--events-only", action="store_true",
                    help="only the file-event row: the main thread's path syscalls while files change under the Code page (lead 10)")
    ap.add_argument("--skip-build", action="store_true")
    ap.add_argument("--no-perf", action="store_true", help="skip perf record even if perf is installed")
    args = ap.parse_args()
    out_dir = args.out_dir
    out_dir.mkdir(parents=True, exist_ok=True)
    if not args.skip_build:
        subprocess.run(["cargo", "build", "-p", "relay-cli", "-p", "relay-native"], cwd=ROOT, check=True)

    result = {"date": datetime.date.today().isoformat(), "size": args.size, "seconds": args.seconds, "perf": bool(shutil.which("perf")) and not args.no_perf,
              "gtk": subprocess.run(["pkg-config", "--modversion", "gtk4"], capture_output=True, text=True).stdout.strip() or None,
              "pages": {}, "notes": []}
    with tempfile.TemporaryDirectory(prefix="relay-native-perf-") as tmp:
        base = Path(tmp)
        runtime = base / "runtime"
        runtime.mkdir(mode=0o700)
        env = dict(os.environ, XDG_DATA_HOME=str(base / "data"), XDG_CONFIG_HOME=str(base / "config"), XDG_CACHE_HOME=str(base / "cache"),
                   XDG_RUNTIME_DIR=str(runtime), RELAY_INSTANCE="test", RELAY_BIN=str(ENGINE), NO_COLOR="1")
        for key in ("RELAY_SESSION", "RELAY_TOKEN", "RELAY_BRIEF"):
            env.pop(key, None)
        provider = base / "provider"
        provider.write_text(PROVIDER)
        provider.chmod(0o755)
        engine_log = open(out_dir / "engine.log", "w")
        engine = subprocess.Popen([str(ENGINE), "--instance", "test", "serve"], env=env, stdout=engine_log, stderr=engine_log)
        conn = socket.socket(socket.AF_UNIX)
        sock_path = runtime / "relay-v4/test.sock"
        try:
            deadline = time.monotonic() + 15
            while True:
                try:
                    conn.connect(str(sock_path))
                    break
                except (FileNotFoundError, ConnectionRefusedError):
                    if engine.poll() is not None or time.monotonic() > deadline:
                        raise RuntimeError("fixture engine failed; see engine.log")
                    time.sleep(0.05)
            stream = conn.makefile("rwb", buffering=0)

            def call(op, payload):
                stream.write((json.dumps(dict(v=1, id=str(uuid.uuid4()), actor="user", op=op, payload=payload)) + "\n").encode())
                while True:
                    line = stream.readline()
                    if not line:
                        raise RuntimeError(f"{op}: the fixture engine closed the connection; see engine.log")
                    msg = json.loads(line)
                    if "ev" in msg or "stream" in msg:
                        continue
                    if not msg["ok"]:
                        raise RuntimeError(f"{op}: {msg['error']}")
                    return msg["result"]

            workspace = base / "workspace"
            repo = workspace / "Native baseline project"
            repo.mkdir(parents=True)
            # Only the repository's own config: a global commit.gpgsign or hooksPath would otherwise
            # sign (or prompt for) the fixture commit and run the developer's hooks on it.
            git_env = dict(os.environ, GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1")
            def git(*a):
                subprocess.run(["git", "-C", str(repo), *a], check=True, capture_output=True, env=git_env)
            git("init", "-q", "-b", "main")
            git("config", "user.name", "fixture")
            git("config", "user.email", "fixture@relay.test")
            (repo / "README.md").write_text("# Native baseline\n\nDisposable.\n")
            for d in range(10):
                (repo / "src" / f"mod{d}").mkdir(parents=True)
                for f in range(20):
                    (repo / "src" / f"mod{d}" / f"file{f}.rs").write_text("".join(f"pub fn f{d}_{f}_{i}() {{}}\n" for i in range(30)))
            git("add", ".")
            git("commit", "-qm", "fixture")
            ws = call("workspace.create", {"path": str(workspace)})
            project = call("project.add", {"workspace_id": ws["id"], "path": str(repo)})
            for name in ("claude", "codex"):
                call("settings.set", {"path": f"providers.{name}.path", "value": str(provider)})
            for i in range(6):
                call("task.create", {"project_id": project["id"], "title": f"Task {i}", "body": "Fixture.", "column": ["ready", "active", "backlog"][i % 3]})
            sessions = []
            for i in range(6):
                s = call("session.create", {"project_id": project["id"], "provider": "claude" if i % 2 == 0 else "codex", "role": "builder"})
                call("session.spawn", {"session": s["name"]})
                sessions.append(s["name"])
            call("notes.create", {"project_id": project["id"], "title": "Baseline", "body": "A note."})
            for name in sessions:
                deadline = time.monotonic() + 10
                while "BASELINE" not in call("session.scrollback", {"session": name, "lines": 5})["text"]:
                    assert time.monotonic() < deadline, name
                    time.sleep(0.05)
            engine_rss = counters(engine.pid)["rss"]
            result["engine_rss_mb_six_sessions"] = round(engine_rss / 1024 / 1024, 1)

            # XDG_RUNTIME_DIR stays the session's own: RELAY_NATIVE_SOCKET already picks the fixture
            # engine, and a relative WAYLAND_DISPLAY is looked up under it, so moving it would send
            # GDK to XWayland (or nowhere) instead of the Wayland path the app really runs on.
            desktop_env = dict(os.environ, RELAY_NATIVE_SOCKET=str(sock_path), RELAY_INSTANCE="test", XDG_DATA_HOME=str(base / "data"),
                               XDG_CONFIG_HOME=str(base / "config"), XDG_CACHE_HOME=str(base / "cache"))
            result["display"] = {key: os.environ.get(key) for key in ("GDK_BACKEND", "WAYLAND_DISPLAY", "DISPLAY", "XDG_SESSION_TYPE")}
            for key in ("RELAY_SESSION", "RELAY_TOKEN", "RELAY_BRIEF"):
                desktop_env.pop(key, None)

            def open_page(label, page, extra_env, seconds, streaming=None):
                shot = out_dir / f"{label}.png"
                shot.unlink(missing_ok=True)
                penv = dict(desktop_env, RELAY_NATIVE_SCREENSHOT=str(shot), RELAY_NATIVE_SIZE=args.size, RELAY_NATIVE_PAGE=page,
                            RELAY_NATIVE_SMOKE_SECONDS=str(seconds + 2 + (0 if streaming else int(args.quiet_after + 0.999)) + EXIT_MARGIN), GDK_DEBUG="frames", **extra_env)
                row = {"page": page}
                with open(out_dir / f"{label}.log", "w") as plog:
                    t0 = time.monotonic()
                    native = subprocess.Popen([str(NATIVE)], env=penv, stdout=plog, stderr=plog)
                    try:
                        # Startup: the first two seconds cover process start, connect, first paint, navigation.
                        time.sleep(2.0)
                        c = counters(native.pid)
                        row["startup"] = {"cpu_ms_first_2s": round(c["cpu_ticks"] / CLK_TCK * 1000, 1), "rss_mb_at_2s": round(c["rss"] / 1024 / 1024, 1), "threads": c["threads"]}
                        if not streaming:
                            time.sleep(args.quiet_after)
                        if streaming:
                            for name in sessions:
                                call("session.input", {"session": name, "data": streaming + "\n"})
                        perf_data = out_dir / f"{label}.perf.data"
                        if result["perf"]:
                            row["steady"], row["profile"] = perf_record(native.pid, seconds, perf_data)
                        else:
                            row["steady"] = window(native.pid, seconds)
                        code = native.wait(timeout=60)
                        if result["perf"] and row["profile"] is None:
                            row["profile"] = perf_fold(perf_data)
                        row["exit_code"] = code
                        row["total_wall_s"] = round(time.monotonic() - t0, 2)
                    finally:
                        if native.poll() is None:
                            native.terminate()
                            native.wait(timeout=5)
                text = (out_dir / f"{label}.log").read_text(errors="replace")
                frames = [l for l in text.splitlines() if "frame" in l.lower() and "gdk" in l.lower()]
                row["gdk_frame_log_lines"] = len(frames)
                row["screenshot"] = shot.is_file()
                return row

            def hidden_row(label, minimize):
                # Minimized (or left on screen) 3 s in, after the startup sample; the requests the
                # client sends from then on are counted by op (RELAY_NATIVE_COUNT_CALLS), with CPU
                # and wakeups over the window after the settle.
                env = {"RELAY_NATIVE_COUNT_CALLS": "1", ("RELAY_NATIVE_MINIMIZE" if minimize else "RELAY_NATIVE_COUNT_FROM"): "3"}
                row = open_page(label, "agents", env, args.hidden_seconds)
                text = (out_dir / f"{label}.log").read_text(errors="replace")
                calls = next((json.loads(l.split("=", 1)[1]) for l in text.splitlines() if l.startswith("CALLS=")), None)
                counted_s = args.hidden_seconds + 2 + int(args.quiet_after + 0.999) + EXIT_MARGIN - 3
                row["calls"] = calls
                row["calls_counted_s"] = counted_s
                row["calls_per_min"] = round(sum((calls or {}).values()) * 60 / counted_s, 2)
                row["state"] = next((l.split("=", 1)[1] for l in text.splitlines() if l.startswith("MINIMIZED_STATE=")), None)
                return row

            def file_events_row(label, touches):
                # The Code page open while `touches` files change, 50 ms apart. The client runs
                # under strace, main thread only (no -f), logging every path lookup with its time:
                # the count inside the window is what each file.changed costs the GTK thread.
                trace = out_dir / f"{label}.strace"
                shot = out_dir / f"{label}.png"
                quiet = int(args.quiet_after + 0.999)
                penv = dict(desktop_env, RELAY_NATIVE_SCREENSHOT=str(shot), RELAY_NATIVE_SIZE=args.size, RELAY_NATIVE_PAGE="code",
                            RELAY_NATIVE_SMOKE_SECONDS=str(2 + quiet + 8 + EXIT_MARGIN))
                cmd = ["strace", "-ttt", "-e", "trace=%stat,%lstat,readlink,readlinkat,statx", "-o", str(trace), str(NATIVE)]
                with open(out_dir / f"{label}.log", "w") as plog:
                    native = subprocess.Popen(cmd, env=penv, stdout=plog, stderr=plog)
                    time.sleep(2 + quiet)
                    start = time.time()
                    for i in range(touches):
                        target = repo / "src" / f"mod{i % 10}" / f"file{i % 20}.rs"
                        target.write_text(target.read_text() + f"// touch {i}\n")
                        time.sleep(0.05)
                    time.sleep(2.5)  # the editor's 1 s debounce, and its reads
                    end = time.time()
                    native.wait(timeout=60)
                calls = 0
                for line in trace.read_text(errors="replace").splitlines():
                    try:
                        at = float(line.split(" ", 1)[0])
                    except ValueError:
                        continue
                    calls += start <= at <= end
                git("checkout", "-q", "--", ".")
                return {"page": "code", "touches": touches, "window_s": round(end - start, 2), "path_syscalls_main_thread": calls,
                        "per_touch": round(calls / touches, 1) if touches else None}

            def session_burst_row(label, events):
                # `events` session.update calls (allow_ui flipped on the fixture's sessions, 50 a
                # second), each answered by a session.changed row the client applies in place:
                # sidebar, wall and status bar. perf counts the GTK thread's user-space
                # instructions over the burst and its settle, against the same window idle.
                quiet = int(args.quiet_after + 0.999)
                shot = out_dir / f"{label}.png"
                penv = dict(desktop_env, RELAY_NATIVE_SCREENSHOT=str(shot), RELAY_NATIVE_SIZE=args.size, RELAY_NATIVE_PAGE="agents",
                            RELAY_NATIVE_SMOKE_SECONDS=str(2 + quiet + 10 + EXIT_MARGIN))
                with open(out_dir / f"{label}.log", "w") as plog:
                    native = subprocess.Popen([str(NATIVE)], env=penv, stdout=plog, stderr=plog)
                    try:
                        time.sleep(2 + quiet)
                        stat_out = out_dir / f"{label}.perfstat"
                        stat = subprocess.Popen(["perf", "stat", "-x,", "-e", "instructions:u,task-clock", "-t", str(native.pid), "-o", str(stat_out), "--", "sleep", "8"])
                        time.sleep(0.3)
                        for i in range(events):
                            call("session.update", {"session": sessions[i % len(sessions)], "allow_ui": bool(i % 2)})
                            time.sleep(float(os.environ.get("RELAY_PERF_BURST_GAP", "0.02")))
                        stat.wait(timeout=30)
                        native.wait(timeout=60)
                    finally:
                        if native.poll() is None:
                            native.terminate()
                            native.wait(timeout=5)
                counts = {}
                for line in stat_out.read_text().splitlines():
                    parts = line.split(",")
                    if len(parts) > 2 and parts[0].replace(".", "", 1).isdigit():
                        counts[parts[2].split(":")[0]] = float(parts[0])
                return {"page": "agents", "events": events, "instructions_u": counts.get("instructions"), "task_clock_ms": counts.get("task-clock")}

            def board_row(label, total):
                # Tasks added up to `total` across the columns, then the board opened six times
                # from the wall: the first is the cold mount, the rest reopen it (PAGE_MS lines).
                have = len(call("task.list", {"project_id": project["id"], "summary": True})["tasks"])
                for i in range(have, total):
                    call("task.create", {"project_id": project["id"], "title": f"Board load task {i} with a title of ordinary length",
                                         "body": "Fixture.", "column": ["backlog", "ready", "active", "in_review", "done"][i % 5]})
                # Three launches, each cold: the saved layout would otherwise reopen the board the
                # last launch left on screen, mounting it before the timed navigation.
                colds, reopen, every = [], [], []
                for launch in range(3):
                    call("settings.set", {"path": f"native.layout.current.{project['id']}", "value": None})
                    shot = out_dir / f"{label}-{launch}.png"
                    penv = dict(desktop_env, RELAY_NATIVE_SCREENSHOT=str(shot), RELAY_NATIVE_SIZE=args.size, RELAY_NATIVE_PAGE="agents",
                                RELAY_NATIVE_TIME_PAGE="board", RELAY_NATIVE_SMOKE_SECONDS="40")
                    cmd = [str(NATIVE)]
                    profile = out_dir / f"{label}.perf.data"
                    if args.board_profile and launch == 0:
                        # From 3.4 s, just before the hook's first navigation (3 s + 0.6 s on the wall).
                        cmd = ["perf", "record", "-e", "cycles:u", "-F", "4999", "--call-graph", "fp", "--user-callchains", "-D", "3400", "-o", str(profile), "--", *cmd]
                    with open(out_dir / f"{label}-{launch}.log", "w") as plog:
                        native = subprocess.Popen(cmd, env=penv, stdout=plog, stderr=plog)
                        native.wait(timeout=90)
                    times = []
                    for line in (out_dir / f"{label}-{launch}.log").read_text(errors="replace").splitlines():
                        if line.startswith("PAGE_MS=board") and "ms=" in line:
                            times.append(float(line.rsplit("ms=", 1)[1]))
                    if times:
                        colds.append(times[0])
                        reopen += times[1:]
                    every.append(times)
                colds.sort()
                reopen.sort()
                folded = perf_fold(out_dir / f"{label}.perf.data") if args.board_profile else None
                return {"profile": folded,"page": "board", "tasks": total, "cold_ms": colds[len(colds) // 2] if colds else None, "cold_ms_all": colds,
                        "reopen_ms_median": reopen[len(reopen) // 2] if reopen else None, "reopen_ms_max": reopen[-1] if reopen else None,
                        "all_ms": [t for times in every for t in times]}

            if args.calls_page:
                for run in range(3):
                    label = f"calls-{args.calls_page}-{run}"
                    penv = dict(desktop_env, RELAY_NATIVE_SCREENSHOT=str(out_dir / f"{label}.png"), RELAY_NATIVE_SIZE=args.size,
                                RELAY_NATIVE_PAGE=args.calls_page, RELAY_NATIVE_COUNT_CALLS="1", RELAY_NATIVE_SMOKE_SECONDS="10")
                    with open(out_dir / f"{label}.log", "w") as plog:
                        subprocess.run([str(NATIVE)], env=penv, stdout=plog, stderr=plog, timeout=90)
                    text = (out_dir / f"{label}.log").read_text(errors="replace")
                    calls = next((json.loads(l.split("=", 1)[1]) for l in text.splitlines() if l.startswith("CALLS=")), None) or {}
                    result["pages"][label] = {"page": args.calls_page, "startup_calls": calls, "startup_total": sum(calls.values())}
                args.pages = ""
                args.hidden_only = True
                args.events_only = True
            elif args.connect:
                label = "connect"
                penv = dict(desktop_env, RELAY_NATIVE_SCREENSHOT=str(out_dir / f"{label}.png"), RELAY_NATIVE_SIZE=args.size, RELAY_NATIVE_PAGE="agents",
                            RELAY_NATIVE_TIME_CONNECT=str(args.connect), RELAY_NATIVE_SMOKE_SECONDS="25")
                with open(out_dir / f"{label}.log", "w") as plog:
                    subprocess.run([str(NATIVE)], env=penv, stdout=plog, stderr=plog, timeout=90)
                row = {"page": "agents", "requests": args.connect}
                for line in (out_dir / f"{label}.log").read_text(errors="replace").splitlines():
                    if line.startswith("CONNECT_US="):
                        fields = dict(f.split("=") for f in line.split()[1:])
                        row[line.split()[0].split("=")[1]] = {k: float(v) for k, v in fields.items()}
                result["pages"][label] = row
                args.pages = ""
                args.hidden_only = True
                args.events_only = True
            elif args.board_tasks:
                for total in [int(n) for n in args.board_tasks.split(",")]:
                    label = f"board-{total}"
                    log(f"{label}: board opening at {total} tasks")
                    try:
                        result["pages"][label] = board_row(label, total)
                    except Exception as e:
                        result["pages"][label] = {"page": "board", "error": str(e)}
                args.pages = ""
                args.hidden_only = True
                args.events_only = True
            elif args.burst_only:
                rows = {}
                for label, events in (("burst-idle", 0), ("burst-200", 200)):
                    log(f"{label}: {events} session.changed on the Agents page")
                    try:
                        rows[label] = result["pages"][label] = session_burst_row(label, events)
                    except Exception as e:
                        result["pages"][label] = {"page": "agents", "error": str(e)}
                idle, busy = rows.get("burst-idle", {}), rows.get("burst-200", {})
                if idle.get("instructions_u") and busy.get("instructions_u"):
                    result["pages"]["burst-200"]["instructions_per_event"] = round((busy["instructions_u"] - idle["instructions_u"]) / 200)
                args.pages = ""
                args.hidden_only = True
                args.events_only = True
            elif args.events_only:
                for label, touches in (("code-events-quiet", 0), ("code-events", 100)):
                    log(f"{label}: {touches} files changed under the Code page")
                    try:
                        result["pages"][label] = file_events_row(label, touches)
                    except Exception as e:
                        result["pages"][label] = {"page": "code", "error": str(e)}
                args.pages = ""
                args.hidden_only = True
            for label, minimize in (("agents-hidden", True), ("agents-shown", False)) if not args.events_only else ():
                log(f"{label}: {args.hidden_seconds} s, requests counted")
                try:
                    result["pages"][label] = hidden_row(label, minimize)
                except Exception as e:
                    result["pages"][label] = {"page": "agents", "error": str(e)}
            if args.hidden_only:
                args.pages = ""
            for page in filter(None, args.pages.split(",")):
                log(f"page {page}")
                try:
                    result["pages"][page] = open_page(page, page, {}, args.seconds)
                except Exception as e:  # keep going: one page's drift must not lose the rest
                    result["pages"][page] = {"page": page, "error": str(e)}
            log("agents page with six terminals each printing 10 lines/s")
            try:
                if args.hidden_only:
                    raise StopIteration
                result["pages"]["agents-streaming"] = open_page("agents-streaming", "agents", {}, args.seconds, streaming=f"tick {args.seconds * 10 + 20} 100")
            except StopIteration:
                pass
            except Exception as e:
                result["pages"]["agents-streaming"] = {"error": str(e)}
            log("burst: 2048 lines into every pane, fixture verification on")
            try:
                if args.hidden_only:
                    raise StopIteration
                result["pages"]["agents-burst"] = open_page("agents-burst", "agents", {"RELAY_NATIVE_FIXTURE": "1", "RELAY_NATIVE_BURST": "1"}, max(args.seconds, 7))
            except StopIteration:
                pass
            except Exception as e:
                result["pages"]["agents-burst"] = {"error": str(e)}
        finally:
            conn.close()
            engine.terminate()
            try:
                engine.wait(timeout=10)
            except subprocess.TimeoutExpired:
                engine.kill()
            engine_log.close()
    (out_dir / "native.json").write_text(json.dumps(result, indent=2) + "\n")
    lines = ["# Native client baseline", "", f"GTK {result.get('gtk')}, window {args.size}, {args.seconds} s per page, perf {'on' if result['perf'] else 'off'}.", "",
             "| page | startup CPU (first 2 s) | RSS at 2 s | steady CPU % | wakeups/s | RSS | threads | exit |", "|---|---:|---:|---:|---:|---:|---:|---:|"]
    for name, row in result["pages"].items():
        if "error" in row:
            lines.append(f"| {name} | error: {row['error'][:80]} | | | | | | |")
            continue
        st, sd = row.get("startup", {}), row.get("steady", {})
        lines.append(f"| {name} | {st.get('cpu_ms_first_2s')} ms | {st.get('rss_mb_at_2s')} MiB | {sd.get('cpu_percent')} | {sd.get('wakeups_per_s')} | {sd.get('rss_mb')} MiB | {sd.get('threads')} | {row.get('exit_code')} |")
    starts = [(n, r) for n, r in result["pages"].items() if "startup_calls" in r]
    if starts:
        lines += ["", "## Requests from launch to 10 s", "", "| run | page | total | by op |", "|---|---|---:|---|"]
        for name, row in starts:
            ops = ", ".join(f"{op} {n}" for op, n in sorted(row["startup_calls"].items()))
            lines.append(f"| {name} | {row['page']} | {row['startup_total']} | {ops} |")
    connect = result["pages"].get("connect")
    if connect and "shared" in connect:
        lines += ["", f"## bus.ping round trip, client side ({connect['requests']} each, µs)", "",
                  "| connection | p50 | p95 | max |", "|---|---:|---:|---:|"]
        for name in ("shared", "own"):
            r = connect.get(name, {})
            lines.append(f"| {name} | {r.get('p50')} | {r.get('p95')} | {r.get('max')} |")
    boards = [(n, r) for n, r in result["pages"].items() if "cold_ms" in r]
    if boards:
        lines += ["", "## Board opening: navigate → first card drawn and laid out", "",
                  "| row | tasks | cold mount, median of 3 launches | each launch's cold | reopen, median | reopen, max |", "|---|---:|---:|---|---:|---:|"]
        for name, row in boards:
            lines.append(f"| {name} | {row['tasks']} | {row['cold_ms']} ms | {', '.join(f'{t:.0f}' for t in row['cold_ms_all'])} | {row['reopen_ms_median']} ms | {row['reopen_ms_max']} ms |")
    bursts = [(n, r) for n, r in result["pages"].items() if "instructions_u" in r]
    if bursts:
        lines += ["", "## GTK thread during a session.changed burst (Agents page, 8 s window)", "",
                  "| row | events | instructions (user) | task-clock | per event, over idle |", "|---|---:|---:|---:|---:|"]
        for name, row in bursts:
            lines.append(f"| {name} | {row['events']} | {row['instructions_u']:,.0f} | {row['task_clock_ms']:.1f} ms | {row.get('instructions_per_event', '')} |")
    events = [(n, r) for n, r in result["pages"].items() if "path_syscalls_main_thread" in r]
    if events:
        lines += ["", "## Path lookups on the GTK thread while files change (Code page)", "",
                  "| row | files changed | window | stat/readlink calls, main thread | per change |", "|---|---:|---:|---:|---:|"]
        for name, row in events:
            lines.append(f"| {name} | {row['touches']} | {row['window_s']} s | {row['path_syscalls_main_thread']} | {row['per_touch']} |")
    hidden = [(n, r) for n, r in result["pages"].items() if "calls_per_min" in r]
    if hidden:
        lines += ["", "## Requests the client sends with the window minimized, and on screen", "",
                  "| row | window state | counted | requests/min | by op |", "|---|---|---:|---:|---|"]
        for name, row in hidden:
            ops = ", ".join(f"{op} {n}" for op, n in sorted((row.get("calls") or {}).items()))
            lines.append(f"| {name} | {row.get('state') or 'shown'} | {row['calls_counted_s']} s | {row['calls_per_min']} | {ops} |")
    for name, row in result["pages"].items():
        prof = row.get("profile")
        if prof and "by_dso" in prof:
            lines += ["", f"## {name}: CPU by shared object", ""] + [f"- {r['what']} **{r['pct']}%**" for r in prof["by_dso"][:12]]
            lines += ["", f"### {name}: hottest symbols", ""] + [f"- `{r['what']}` {r['pct']}%" for r in prof.get("by_symbol", [])[:20]]
    (out_dir / "report.md").write_text("\n".join(lines) + "\n")
    log(f"done: {out_dir}")


if __name__ == "__main__":
    main()
