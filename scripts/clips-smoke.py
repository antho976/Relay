#!/usr/bin/env python3
"""The Clips space against a disposable engine, captured by relay-native.

    python3 scripts/clips-smoke.py [--from DIR] [shot ...]

Without --from, the engine starts empty: the Overview's empty state and the import sheet. With
--from DIR (a store folder of a Clips run: `store.db`, `clips.db` and `videos/`, as
`relay --instance test serve --store DIR/store.db` leaves it), its two files are copied into the
disposable engine (VODs left working are stopped, so nothing runs again) and its videos folder is
linked, so the pages show real VODs, clips and posters.

Shots (default all): clips-home, clips-vod-<id>, clips-clip-<id> (the first VOD and its best clip),
clips-queue, clips-settings, clips-import, clips-start. Needs target/debug/relay and
target/debug/relay-native, and a display: run it inside a headless compositor (kwin_wayland
--virtual), never on someone's desktop. Captures and logs go to .impeccable/review/clips-*. No
model is called, nothing is posted, and nothing reaches the person's own engine."""
import json, os, socket, sqlite3, subprocess, sys, tempfile, time, uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / ".impeccable/review"; OUT.mkdir(parents=True, exist_ok=True)
ENGINE = ROOT / "target/debug/relay"
NATIVE = ROOT / "target/debug/relay-native"

args = sys.argv[1:]
source = None
if args[:1] == ["--from"]:
    source = Path(args[1]).resolve()
    args = args[2:]


def copy_db(src: Path, dst: Path):
    """A consistent copy of a live SQLite file (it may be open in WAL mode)."""
    with sqlite3.connect(f"file:{src}?mode=ro", uri=True) as a, sqlite3.connect(dst) as b:
        a.backup(b)


with tempfile.TemporaryDirectory(prefix="relay-clips-shot-") as temporary:
    base = Path(temporary)
    runtime = base / "runtime"; runtime.mkdir(mode=0o700)
    data = base / "data/relay-v4/test"; data.mkdir(parents=True)
    if source:
        copy_db(source / "store.db", data / "store.db")
        copy_db(source / "clips.db", data / "clips.db")
        with sqlite3.connect(data / "clips.db") as c:
            c.execute("UPDATE vod SET state = 'stopped' WHERE state IN ('working', 'queued')")
            c.execute("UPDATE clip SET state = 'failed', error = 'Stopped for the capture' WHERE state IN ('rendering', 'posting')")
        (data / "videos").symlink_to(source / "videos")
    env = dict(os.environ, XDG_DATA_HOME=str(base / "data"), XDG_CONFIG_HOME=str(base / "config"),
               XDG_CACHE_HOME=str(base / "cache"), XDG_RUNTIME_DIR=str(runtime), RELAY_INSTANCE="test",
               RELAY_BIN=str(ENGINE), NO_COLOR="1")
    for key in ("RELAY_SESSION", "RELAY_TOKEN", "RELAY_BRIEF", "WAYLAND_DISPLAY"):
        env.pop(key, None)
    log = (OUT / "clips-engine.log").open("w")
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
                    raise RuntimeError("engine failed; see clips-engine.log")
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

        vods = call("clips.vod.list", {})["vods"]
        shots = args or ["clips-home", "clips-queue", "clips-settings", "clips-import", "clips-start"]
        if vods and not args:
            vod = vods[-1]
            clips = call("clips.vod.get", {"id": vod["id"]})["clips"]
            shots.insert(1, f"clips-vod-{vod['id']}")
            drawn = [c for c in clips if c.get("file")] or clips
            if drawn:
                shots.insert(2, f"clips-clip-{drawn[0]['id']}")

        native_env = dict(os.environ, RELAY_NATIVE_SOCKET=str(path), RELAY_INSTANCE="test", XDG_CACHE_HOME=str(base / "native-cache"))
        for key in ("RELAY_SESSION", "RELAY_TOKEN", "RELAY_BRIEF"):
            native_env.pop(key, None)
        for shot in shots:
            output = OUT / f"{shot}.png"; output.unlink(missing_ok=True)
            page = "start" if shot == "clips-start" else shot
            e = dict(native_env, RELAY_NATIVE_SCREENSHOT=str(output), RELAY_NATIVE_SIZE="1440,900", RELAY_NATIVE_FIXTURE="1",
                     RELAY_NATIVE_PAGE=page, RELAY_NATIVE_SMOKE_SECONDS="7")
            if shot == "clips-start":
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
