#!/usr/bin/env python3
"""Thumbnail reads when a Threads conversation with many pictures is opened, left and reopened.

A disposable engine (temporary XDG dirs, instance "test", a fake claude that fails at once) holds
a thread of THUMBS_MESSAGES (default 30) messages with four 2000x1500 JPEGs each, and a second
thread with none. relay-native opens the first, then the second, then the first again
(smoke.rs, RELAY_NATIVE_PAGE=thumbs) and prints a THUMBS line for each opening: thumbnails read,
pictures on screen, how many of those were read, and the ms from opening until every picture
on screen shows.

    cargo build -p relay-cli -p relay-native
    python3 scripts/perf/thumbs-reopen.py [runs]

It opens a window, so run it inside a private compositor, never on a desktop in use
(`kwin_wayland --virtual --no-lockscreen --socket=...`, then WAYLAND_DISPLAY=<socket>). It never
touches the user's store and never launches a model. FIXES-2026-10-19.md has the reading.
"""
import base64, io, json, os, random, socket, subprocess, sys, tempfile, time, uuid
from pathlib import Path
from PIL import Image

# The waits here end on assert statements; under -O a stalled wait would never end.
if sys.flags.optimize:
    raise SystemExit("Run without python -O or PYTHONOPTIMIZE: this script times out with assert.")

ROOT = Path(__file__).resolve().parents[2]
RUNS = int(sys.argv[1]) if len(sys.argv) > 1 else 5
MESSAGES = int(os.environ.get("THUMBS_MESSAGES", "30"))
ENGINE = ROOT / "target/debug/relay"
NATIVE = ROOT / "target/debug/relay-native"


def photo(seed):
    """A 2000x1500 JPEG with enough detail to weigh what a photo does."""
    rnd = random.Random(seed)
    im = Image.new("RGB", (2000, 1500), (rnd.randrange(256), rnd.randrange(256), rnd.randrange(256)))
    px = im.load()
    for _ in range(4000):
        x, y = rnd.randrange(1990), rnd.randrange(1490)
        c = (rnd.randrange(256), rnd.randrange(256), rnd.randrange(256))
        for dx in range(10):
            for dy in range(10):
                px[x + dx, y + dy] = c
    out = io.BytesIO()
    im.save(out, "JPEG", quality=85)
    return out.getvalue()


with tempfile.TemporaryDirectory(prefix="relay-thumbs-") as temporary:
    base = Path(temporary)
    runtime = base / "runtime"
    runtime.mkdir(mode=0o700)
    env = dict(os.environ, XDG_DATA_HOME=str(base / "data"), XDG_CONFIG_HOME=str(base / "config"),
               XDG_CACHE_HOME=str(base / "cache"), XDG_RUNTIME_DIR=str(runtime), RELAY_INSTANCE="test",
               RELAY_BIN=str(ENGINE), NO_COLOR="1")
    for key in ("RELAY_SESSION", "RELAY_TOKEN", "RELAY_BRIEF"):
        env.pop(key, None)
    provider = base / "claude"
    provider.write_text("#!/usr/bin/env python3\nimport sys\nif '--version' in sys.argv: print('fake 1.0')\nsys.exit(1)\n")
    provider.chmod(0o755)
    log = (base / "engine.log").open("w")
    engine = subprocess.Popen([str(ENGINE), "--instance", "test", "serve"], env=env, stdout=log, stderr=log)
    path = runtime / "relay-v4/test.sock"
    connection = socket.socket(socket.AF_UNIX)
    try:
        deadline = time.monotonic() + 15
        while True:
            try:
                connection.connect(str(path))
                break
            except (FileNotFoundError, ConnectionRefusedError):
                assert engine.poll() is None and time.monotonic() < deadline, "engine failed"
                time.sleep(0.05)
        stream = connection.makefile("rwb", buffering=0)

        def call(op, payload):
            stream.write((json.dumps(dict(v=1, id=str(uuid.uuid4()), actor="user", op=op, payload=payload)) + "\n").encode())
            while True:
                response = json.loads(stream.readline())
                if "ok" in response:
                    break
            if not response["ok"]:
                raise RuntimeError(f"{op}: {response['error']}")
            return response["result"]

        call("settings.set", {"path": "providers.claude.path", "value": str(provider)})

        def settled(thread):
            for _ in range(200):
                if not call("thread.get", {"id": thread})["thread"]["working"]:
                    return
                time.sleep(0.05)
            raise RuntimeError("thread kept working")

        thread = None
        for m in range(MESSAGES):
            ids = []
            for k in range(4):
                up = call("thread.upload", {"media_type": "image/jpeg", "bytes_b64": base64.b64encode(photo(m * 4 + k)).decode()})
                ids.append(up["image"]["id"])
            if thread is None:
                thread = call("thread.create", {"text": f"Message {m}", "images": ids})["id"]
            else:
                call("thread.send", {"id": thread, "text": f"Message {m}", "images": ids})
            settled(thread)
        other = call("thread.create", {"text": "No pictures here"})["id"]
        settled(other)
        read = call("thread.get", {"id": thread})
        pictures = sum(len(m["body"].get("images", [])) for m in read["messages"] if m["role"] == "user")
        print(json.dumps({"thread": thread, "other": other, "messages": len(read["messages"]), "pictures": pictures}), flush=True)
        if os.environ.get("THUMBS_SEED_ONLY"):
            raise SystemExit(0)
        for run in range(RUNS):
            out = base / f"thumbs-{run}.png"
            # The window keeps the session's own runtime dir, where the compositor's socket is.
            native_env = dict(os.environ, RELAY_INSTANCE="test", RELAY_NATIVE_SOCKET=str(path), RELAY_NATIVE_SCREENSHOT=str(out), RELAY_NATIVE_SIZE="1440,900",
                              RELAY_NATIVE_PAGE="thumbs", RELAY_NATIVE_THUMBS=f"{thread},{other}", RELAY_NATIVE_SMOKE_SECONDS="4")
            for key in ("RELAY_SESSION", "RELAY_TOKEN", "RELAY_BRIEF"):
                native_env.pop(key, None)
            done = subprocess.run([str(NATIVE)], env=native_env, capture_output=True, text=True, timeout=180)
            lines = [l for l in (done.stdout + done.stderr).splitlines() if l.startswith("THUMBS") or "panicked" in l]
            print("\n".join(lines) or (done.stdout + done.stderr)[-2000:], flush=True)
    finally:
        engine.terminate()
        engine.wait(timeout=10)
