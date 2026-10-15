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

            for page in args.pages.split(","):
                log(f"page {page}")
                try:
                    result["pages"][page] = open_page(page, page, {}, args.seconds)
                except Exception as e:  # keep going: one page's drift must not lose the rest
                    result["pages"][page] = {"page": page, "error": str(e)}
            log("agents page with six terminals each printing 10 lines/s")
            try:
                result["pages"]["agents-streaming"] = open_page("agents-streaming", "agents", {}, args.seconds, streaming=f"tick {args.seconds * 10 + 20} 100")
            except Exception as e:
                result["pages"]["agents-streaming"] = {"error": str(e)}
            log("agents page, window minimized-equivalent is not scriptable here; skipped")
            result["notes"].append("Hidden-window detach (PR #13) needs a compositor and is not measured by this script.")
            log("burst: 2048 lines into every pane, fixture verification on")
            try:
                result["pages"]["agents-burst"] = open_page("agents-burst", "agents", {"RELAY_NATIVE_FIXTURE": "1", "RELAY_NATIVE_BURST": "1"}, max(args.seconds, 7))
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
    for name, row in result["pages"].items():
        prof = row.get("profile")
        if prof and "by_dso" in prof:
            lines += ["", f"## {name}: CPU by shared object", ""] + [f"- {r['what']} **{r['pct']}%**" for r in prof["by_dso"][:12]]
            lines += ["", f"### {name}: hottest symbols", ""] + [f"- `{r['what']}` {r['pct']}%" for r in prof.get("by_symbol", [])[:20]]
    (out_dir / "report.md").write_text("\n".join(lines) + "\n")
    log(f"done: {out_dir}")


if __name__ == "__main__":
    main()
