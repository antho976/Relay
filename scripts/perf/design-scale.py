#!/usr/bin/env python3
"""What another window's edit costs the native client on a large design (TM-80, B85), in place of
teal-moose's lost `bigperf.py`: docs/perf/DESIGN-2026-10-20-scale.md.

    python3 scripts/perf/design-scale.py [--boards 40 114] [--edits 40] [--out FILE]

For each design size (`design-smoke.py perf`'s shape: phone artboards in auto layout, each a
title and ten rows of a text and a toggle; 40 artboards are 1,721 nodes and 114 are 4,903) and
each view (the gallery alone, and the board open on the design), it starts relay-native against
a disposable engine, lets it settle, reads its CPU at rest, then sends one `design.apply`
updating one text every 200 ms as the user, which is what another window does, and reads the
CPU again. The figures are CPU per edit, the rest rate taken out: the GTK main thread's (its
schedstat, in nanoseconds) and the whole process's (`/proc/<pid>/stat`, in clock ticks, threads
that ended included), and the medians of the client's `DESIGN_TRACE` lines
(`RELAY_DESIGN_TRACE`: each reread and its phases, layout, content, draw, gallery, thumbnail).
Teal-moose's TM-80 reading (FOUND-teal-moose.md) took the same figures with the same edits.

The socket must be named relay-headless-*, as docs/perf/PLAYBOOK.md starts it. Needs target/debug/relay and target/debug/relay-native (dev profile, what run.sh launches) and
a display: run it inside a headless compositor, never on someone's desktop:

    kwin_wayland --virtual --no-lockscreen --socket=$SOCK --width 1440 --height 900 &
    WAYLAND_DISPLAY=$SOCK DISPLAY= GDK_BACKEND=wayland python3 scripts/perf/design-scale.py

`--engine` and `--native` take other builds (an older commit's, for a before).
"""
import argparse, json, os, socket, statistics, subprocess, sys, tempfile, time, uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def node(id, parent, kind, **kw):
    n = {"id": id, "parent": parent, "kind": kind}
    n.update(kw)
    return {"op": "add", "node": n}


def artboards(page, boards):
    """design-smoke.py's perf design, `boards` artboards of 43 nodes each."""
    ops = []
    for a in range(boards):
        ab = f"a{a}"
        ops.append(node(ab, page, "frame", name=f"Screen {a + 1}", x=(a % 8) * 470, y=(a // 8) * 924, w=390, h=844,
                        fill="#F4F2EE", layout="column", gap=12, padding=[48, 20]))
        ops.append(node(f"{ab}-t", ab, "text", text=f"Screen {a + 1}", font_size=28, font_weight=700))
        ops.append(node(f"{ab}-l", ab, "frame", fill="#FFFFFF", radius=14, layout="column", padding=[4, 14], w_sizing="fill", h_sizing="hug"))
        for r in range(10):
            row = f"{ab}-r{r}"
            ops += [node(row, f"{ab}-l", "frame", fill="#00000000", layout="row", items="center", justify="space_between",
                         padding=[12, 0], w_sizing="fill", h_sizing="hug"),
                    node(f"{row}-x", row, "text", text=f"Setting number {r + 1}", font_size=15),
                    node(f"{row}-s", row, "frame", w=46, h=28, radius=14, fill="#2EC469", layout="row", padding=[3], justify="end"),
                    node(f"{row}-k", f"{row}-s", "ellipse", w=22, h=22, fill="#FFFFFF")]
    return ops


TICK_NS = 1e9 / os.sysconf("SC_CLK_TCK")


def cpu(pid):
    """Nanoseconds on a CPU so far: the main thread's (its schedstat), and the whole process's,
    threads that have ended included (utime and stime, in clock ticks)."""
    main = int(Path(f"/proc/{pid}/task/{pid}/schedstat").read_text().split()[0])
    # The fields after the command's closing parenthesis; utime and stime are the 12th and 13th.
    fields = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
    return main, (int(fields[11]) + int(fields[12])) * TICK_NS


def medians(log, kind, fields):
    """The median of each numbered field of the `DESIGN_TRACE <kind>` lines."""
    rows = [l.split() for l in log.splitlines() if l.startswith(f"DESIGN_TRACE {kind} ")]
    out = {"n": len(rows)}
    for name, at in fields.items():
        values = [float(r[at]) for r in rows if len(r) > at]
        if values:
            out[name] = round(statistics.median(values), 3)
    return out


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--boards", type=int, nargs="+", default=[40, 114])
    parser.add_argument("--views", nargs="+", default=["gallery", "board"])
    parser.add_argument("--edits", type=int, default=40)
    parser.add_argument("--every", type=float, default=0.2, help="seconds between edits")
    parser.add_argument("--rest", type=float, default=3.0, help="seconds of rest measured before the edits")
    parser.add_argument("--engine", type=Path, default=ROOT / "target/debug/relay")
    parser.add_argument("--native", type=Path, default=ROOT / "target/debug/relay-native")
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    if "relay-headless" not in os.environ.get("WAYLAND_DISPLAY", ""):
        sys.exit("run inside a headless compositor whose socket is named relay-headless-* (see the docstring)")
    results = []
    with tempfile.TemporaryDirectory(prefix="relay-design-scale-") as temporary:
        base = Path(temporary)
        runtime = base / "runtime"; runtime.mkdir(mode=0o700)
        env = dict(os.environ, XDG_DATA_HOME=str(base / "data"), XDG_CONFIG_HOME=str(base / "config"),
                   XDG_CACHE_HOME=str(base / "cache"), XDG_RUNTIME_DIR=str(runtime), RELAY_INSTANCE="test", NO_COLOR="1")
        for key in ("RELAY_SESSION", "RELAY_TOKEN", "RELAY_BRIEF"):
            env.pop(key, None)
        engine_env = dict(env); engine_env.pop("WAYLAND_DISPLAY", None)
        log = (base / "engine.log").open("w")
        engine = subprocess.Popen([str(args.engine), "--instance", "test", "serve"], env=engine_env, stdout=log, stderr=log)
        path = runtime / "relay-v4/test.sock"
        conn = socket.socket(socket.AF_UNIX)
        try:
            deadline = time.monotonic() + 20
            while True:
                try:
                    conn.connect(str(path)); break
                except (FileNotFoundError, ConnectionRefusedError):
                    if engine.poll() is not None or time.monotonic() > deadline:
                        sys.exit("the engine did not start: " + (base / "engine.log").read_text()[-2000:])
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

            ws_dir = base / "workspace"; repo = ws_dir / "app"; repo.mkdir(parents=True)
            for git in (["init", "-q", "-b", "main"], ["config", "user.name", "f"], ["config", "user.email", "f@x"]):
                subprocess.run(["git", "-C", str(repo), *git], check=True)
            (repo / "README.md").write_text("# fixture\n")
            subprocess.run(["git", "-C", str(repo), "add", "."], check=True)
            subprocess.run(["git", "-C", str(repo), "commit", "-qm", "fixture"], check=True)
            ws = call("workspace.create", {"path": str(ws_dir)})
            pid = call("project.add", {"workspace_id": ws["id"], "path": str(repo)})["id"]
            # As design-smoke.py starts it: the real XDG_RUNTIME_DIR, where the compositor's socket is.
            native_env = dict(os.environ, RELAY_NATIVE_SOCKET=str(path), RELAY_INSTANCE="test", RELAY_NATIVE_SIZE="1440,900",
                              RELAY_NATIVE_PAGE="design", RELAY_DESIGN_TRACE="1", XDG_CACHE_HOME=str(base / "native-cache"))
            for key in ("RELAY_SESSION", "RELAY_TOKEN", "RELAY_BRIEF"):
                native_env.pop(key, None)
            for boards in args.boards:
                made = call("design.create", {"project_id": pid, "title": f"Scale {boards}"})
                design, page = made["design"]["id"], made["nodes"][0]["id"]
                ops = artboards(page, boards)
                for at in range(0, len(ops), 2000):
                    nodes = call("design.apply", {"design_id": design, "ops": ops[at:at + 2000]})["design"]["nodes"]
                for view in args.views:
                    # The board opens the gallery's first card: the design changed last.
                    call("design.update", {"design_id": design, "title": f"Scale {boards}"})
                    seconds = 8 + args.rest + args.edits * args.every + 4
                    e = dict(native_env, RELAY_NATIVE_SCREENSHOT=str(base / f"{boards}-{view}.png"), RELAY_NATIVE_SMOKE_SECONDS=str(int(seconds)))
                    if view == "board":
                        e["RELAY_SMOKE_DESIGN_BOARD"] = "1"
                    out_path = base / f"{boards}-{view}.log"
                    with out_path.open("w") as out:
                        native = subprocess.Popen([str(args.native)], env=e, stdout=out, stderr=out)
                        try:
                            time.sleep(8)
                            rest0, t0 = cpu(native.pid), time.monotonic()
                            time.sleep(args.rest)
                            rest1, t1 = cpu(native.pid), time.monotonic()
                            for i in range(args.edits):
                                started = time.monotonic()
                                call("design.apply", {"design_id": design, "ops": [{"op": "update", "id": "a3-r4-x", "set": {"text": f"Changed {boards} {view} {i}"}}]})
                                time.sleep(max(0.0, args.every - (time.monotonic() - started)))
                            time.sleep(1.0)
                            busy1, t2 = cpu(native.pid), time.monotonic()
                        finally:
                            native.terminate()
                            try:
                                native.wait(timeout=10)
                            except subprocess.TimeoutExpired:
                                native.kill()
                    text = out_path.read_text()
                    rest = [(b - a) / (t1 - t0) for a, b in zip(rest0, rest1)]
                    per_edit = [((b - a) - r * (t2 - t1)) / args.edits / 1e6 for a, b, r in zip(rest1, busy1, rest)]
                    row = {
                        "nodes": nodes, "view": view, "edits": args.edits,
                        "ui_thread_ms_per_edit": round(per_edit[0], 2), "process_ms_per_edit": round(per_edit[1], 2),
                        "rest_ui_thread_pct": round(rest[0] / 1e7, 3), "rest_process_pct": round(rest[1] / 1e7, 3),
                        "reread": medians(text, "reread", {"ms": 2, "fetch": 7, "decode": 9, "build": 11, "diff": 13}),
                        "commit": medians(text, "commit", {"ms": 2}),
                        "layout": medians(text, "layout", {"ms": 2}),
                        "content": medians(text, "content", {"ms": 2}),
                        "draw": medians(text, "draw", {"ms": 2}),
                        "gallery_render": medians(text, "gallery render", {"ms": 3}),
                        "thumb": medians(text, "thumb", {"ms": 3, "fetch": 6, "decode": 8, "layout": 10, "paint": 12, "texture": 14}),
                        "whole_rereads": text.count("reading it whole"),
                    }
                    results.append(row)
                    print(json.dumps(row), flush=True)
        finally:
            engine.terminate()
            try:
                engine.wait(timeout=10)
            except subprocess.TimeoutExpired:
                engine.kill()
    if args.out:
        args.out.write_text("".join(json.dumps(r) + "\n" for r in results))


if __name__ == "__main__":
    main()
