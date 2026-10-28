#!/usr/bin/env python3
"""Videos in the Design view against a disposable engine (docs/VIDEO.md): two videos made,
checked, looked at and one rendered through the real worker, with a second cut, a named version
and a diff, then relay-native's captures of the gallery's Videos section, each video's board, and
two renders compared.

    python3 scripts/video-smoke.py

Needs target/debug/relay and target/debug/relay-native (cargo build -p relay-cli -p
relay-native), kwin_wayland, and the video kit the end-to-end test sets up once
(`cargo test -p relay-core --test all video_e2e -- --ignored`), whose node_modules it borrows so
it downloads nothing. Every window opens in a private `kwin_wayland --virtual`, never on a
desktop. The engine's files are kept under target/video-smoke/ (on disk: /tmp is memory);
captures and logs go to .impeccable/review/video-*. Exits non-zero when a step fails."""
import json, os, shutil, socket, subprocess, sys, tempfile, time, uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / ".impeccable/review"; OUT.mkdir(parents=True, exist_ok=True)
ENGINE, NATIVE = ROOT / "target/debug/relay", ROOT / "target/debug/relay-native"
KIT = ROOT / "target/video-e2e/store/videos"
for need in (ENGINE, NATIVE, KIT / "node_modules/remotion", KIT / ".relay/kit.json"):
    if not need.exists():
        sys.exit(f"{need} is missing: see this script's docstring")

base = ROOT / "target/video-smoke" / str(os.getpid())
# The socket's folder: short (a Unix socket's path is at most 107 bytes), and under /tmp, where
# relay-native's smoke settings require a disposable engine to be. It holds only the socket.
runtime = Path(tempfile.mkdtemp(prefix="relay-video-smoke-", dir="/tmp"))
data = base / "data/relay-v4/test"; videos = data / "videos"; (videos / ".relay").mkdir(parents=True)
# The kit, borrowed: its packages, and what says they are set up.
os.symlink(KIT / "node_modules", videos / "node_modules")
shutil.copy(KIT / "package.json", videos / "package.json")
shutil.copy(KIT / ".relay/kit.json", videos / ".relay/kit.json")

env = dict(os.environ, XDG_DATA_HOME=str(base / "data"), XDG_CONFIG_HOME=str(base / "config"), XDG_CACHE_HOME=str(base / "cache"),
           XDG_RUNTIME_DIR=str(runtime), RELAY_INSTANCE="test", RELAY_BIN=str(ENGINE), NO_COLOR="1")
for key in ("RELAY_SESSION", "RELAY_TOKEN", "RELAY_BRIEF", "WAYLAND_DISPLAY"):
    env.pop(key, None)
log = (OUT / "video-engine.log").open("w")
engine = subprocess.Popen([str(ENGINE), "--instance", "test", "serve"], env=env, stdout=log, stderr=log)
path = runtime / "relay-v4/test.sock"
conn = socket.socket(socket.AF_UNIX)
kwin = None
failed = []
try:
    deadline = time.monotonic() + 20
    while True:
        try:
            conn.connect(str(path)); break
        except (FileNotFoundError, ConnectionRefusedError):
            if engine.poll() is not None or time.monotonic() > deadline:
                raise RuntimeError("engine failed; see video-engine.log")
            time.sleep(0.05)
    stream = conn.makefile("rwb")

    def call(op, payload):
        ask = dict(v=1, id=str(uuid.uuid4()), actor="user", op=op, payload=payload)
        stream.write((json.dumps(ask) + "\n").encode()); stream.flush()
        while True:
            r = json.loads(stream.readline())
            if "ok" in r:
                break
        if not r["ok"]:
            raise RuntimeError(f"{op}: {r['error']}")
        return r["result"]

    def timed(what, op, payload):
        t0 = time.monotonic()
        out = call(op, payload)
        print(f"{what}: {round((time.monotonic() - t0) * 1000)} ms")
        return out

    ws = base / "workspace"; repo = ws / "Tally"; repo.mkdir(parents=True)
    subprocess.run(["git", "init", "-q", str(repo)], check=True)
    (repo / "README.md").write_text("# Tally\n")
    subprocess.run(["git", "-C", str(repo), "add", "."], check=True)
    subprocess.run(["git", "-C", str(repo), "-c", "user.name=smoke", "-c", "user.email=smoke@relay", "-c", "commit.gpgsign=false",
                    "commit", "-q", "-m", "init"], check=True)
    call("workspace.create", {"path": str(ws)})
    call("project.add", {"workspace_id": 1, "path": str(repo)})
    call("plugin.enable", {"plugin_id": "video", "project_id": 1, "enabled": True})
    assert call("video.kit.status", {})["state"] == "ready", "the borrowed kit does not read as set up"

    promo = call("video.create", {"project_id": 1, "title": "Tally launch promo", "seconds": 6})["video"]["id"]
    checked = timed("check", "video.check", {"video_id": promo})
    assert checked["ok"], checked
    sheet = timed("sheet", "video.still", {"video_id": promo, "count": 6})
    print("issues", json.dumps(sheet["issues"]))
    call("video.update", {"video_id": promo, "props": {"main": {"title": "Every euro in its place", "cta": "Get Tally", "accent": "#F5A524"}}})
    # A music bed, so the render has sound to measure and bring to YouTube's loudness.
    import math, struct, wave
    bed = base / "bed.wav"
    with wave.open(str(bed), "wb") as w:
        w.setnchannels(2); w.setsampwidth(2); w.setframerate(48000)
        frames = bytearray()
        for i in range(48000 * 6):
            t = i / 48000
            v = int(32767 * 0.25 * math.sin(2 * math.pi * 220 * t) * (0.6 + 0.4 * math.sin(2 * math.pi * 0.5 * t)))
            frames += struct.pack("<hh", v, v)
        w.writeframes(bytes(frames))
    call("video.asset.add", {"video_id": promo, "path": str(bed)})
    video_tsx = Path(call("video.get", {"video_id": promo})["video"]["dir"]) / "src/Video.tsx"
    code = video_tsx.read_text()
    code = code.replace("import { Glow,", "import { Music, Glow,", 1).replace("    <TransitionSeries>\n", "    <>\n    <Music src=\"assets/bed.wav\" fadeOut={15} />\n    <TransitionSeries>\n", 1)
    code = code.replace("    </TransitionSeries>\n  );\n};", "    </TransitionSeries>\n    </>\n  );\n};", 1)
    video_tsx.write_text(code)
    checked = call("video.check", {"video_id": promo})
    assert checked["ok"] and not checked["errors"], checked
    t0 = time.monotonic()
    done = call("video.render", {"video_id": promo, "quality": "preview", "preset": "youtube", "name": "youtube", "wait": True})
    print(f"render: {round((time.monotonic() - t0) * 1000)} ms, {done['state']}, {done.get('bytes')} bytes, sound {json.dumps(done.get('audio'))}")
    assert done["state"] == "done", done
    assert abs(done["audio"]["lufs"] + 14) < 0.6, done["audio"]
    timeline = timed("timeline", "video.timeline", {"video_id": promo})
    print("timeline", timeline["png"], json.dumps([s["name"] for s in timeline["sounds"]]))
    # Cuts and takes: a square cut beside the vertical one, the version with both named, a render
    # of each and a second of the vertical, so the board has cuts, a kept version and two renders
    # of one cut to compare.
    root_tsx = video_tsx.parent / "Root.tsx"
    root = root_tsx.read_text()
    root = root.replace("export const Root: React.FC = () => (\n  <Composition", "export const Root: React.FC = () => (\n  <>\n  <Composition", 1)
    root = root.replace("  />\n);", "  />\n  <Composition id=\"square\" component={Video} schema={schema} width={1080} height={1080} fps={30} durationInFrames={180}\n"
                        "    defaultProps={{ title: 'Every euro, square', subtitle: 'For the feed', cta: 'Get Tally', accent: '#34D399' }} />\n  </>\n);", 1)
    root_tsx.write_text(root)
    checked = call("video.check", {"video_id": promo})
    assert checked["ok"] and [c["id"] for c in checked["compositions"]] == ["main", "square"], checked
    named = call("video.version.label", {"video_id": promo, "version": checked["version"], "label": "Two cuts: vertical and square"})
    assert named["label"] == "Two cuts: vertical and square", named
    square = call("video.render", {"video_id": promo, "composition": "square", "quality": "preview", "wait": True})
    assert square["state"] == "done", square
    vertical = call("video.render", {"video_id": promo, "quality": "preview", "wait": True})
    assert vertical["state"] == "done", vertical
    diff = timed("diff since v1", "video.version.diff", {"video_id": promo, "from": 1})
    print("diff", json.dumps([(f["path"], f["change"]) for f in diff["files"]]))
    assert any(f["path"] == "src/Root.tsx" and "square" in (f.get("diff") or "") for f in diff["files"]), diff
    recap = call("video.create", {"project_id": 1, "title": "Avex weekly recap", "shape": "landscape", "seconds": 8})["video"]["id"]
    call("video.check", {"video_id": recap})
    call("video.still", {"video_id": recap, "at": ["2s"]})

    sock = f"relay-video-smoke-{os.getpid()}"
    kwin = subprocess.Popen(["kwin_wayland", "--virtual", "--no-lockscreen", f"--socket={sock}", "--width", "1440", "--height", "900"],
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    real_runtime = Path(os.environ.get("XDG_RUNTIME_DIR", f"/run/user/{os.getuid()}"))
    deadline = time.monotonic() + 20
    while not (real_runtime / sock).exists():
        assert kwin.poll() is None and time.monotonic() < deadline, "kwin_wayland --virtual did not start"
        time.sleep(0.05)
    native_env = dict(os.environ, RELAY_NATIVE_SOCKET=str(path), RELAY_INSTANCE="test", XDG_CACHE_HOME=str(base / "native-cache"),
                      XDG_CONFIG_HOME=str(base / "native-config"), WAYLAND_DISPLAY=sock, DISPLAY="", GDK_BACKEND="wayland")
    for key in ("RELAY_SESSION", "RELAY_TOKEN", "RELAY_BRIEF"):
        native_env.pop(key, None)
    # No sound: the virtual compositor hides the window, not what it plays. The board mutes its
    # media in a screenshot run; no audio server is reachable either.
    native_env.update(PULSE_SERVER="unix:/nonexistent", PIPEWIRE_REMOTE="/nonexistent")

    def native(shot, seconds=7, **extra):
        output = OUT / f"video-{shot}.png"; output.unlink(missing_ok=True)
        e = dict(native_env, RELAY_NATIVE_SCREENSHOT=str(output), RELAY_NATIVE_SIZE="1440,900", RELAY_NATIVE_FIXTURE="1",
                 RELAY_NATIVE_PAGE="design", RELAY_NATIVE_SMOKE_SECONDS=str(seconds))
        e.update({k: str(v) for k, v in extra.items()})
        log_path = OUT / f"video-{shot}.log"
        with log_path.open("w") as nl:
            proc = subprocess.Popen([str(NATIVE)], env=e, stdout=nl, stderr=nl)
            try:
                proc.wait(timeout=seconds + 45)
            finally:
                if proc.poll() is None:
                    proc.terminate(); proc.wait(timeout=5)
        text = log_path.read_text()
        assert "panicked" not in text, text[-3000:]
        assert output.is_file() and "Screenshot saved" in text, ("no capture", text[-2000:])
        print(shot, "->", output)

    for shot, extra in [("gallery", {}), ("board", {"RELAY_SMOKE_VIDEO_ID": promo}), ("board-unrendered", {"RELAY_SMOKE_VIDEO_ID": recap}),
                        ("board-playing", {"RELAY_SMOKE_VIDEO_ID": promo, "RELAY_SMOKE_VIDEO_PLAY": 1}),
                        ("board-compare", {"RELAY_SMOKE_VIDEO_ID": promo, "RELAY_SMOKE_VIDEO_COMPARE": 1}),
                        ("board-versions", {"RELAY_SMOKE_VIDEO_ID": promo, "RELAY_SMOKE_VIDEO_VERSIONS": 1})]:
        try:
            native(shot, **extra)
        except Exception as e:  # noqa: BLE001 - each shot fails on its own
            failed.append(shot)
            print(shot, "FAILED", e)
finally:
    if kwin:
        kwin.terminate()
    engine.terminate()
    try:
        engine.wait(timeout=10)
    except subprocess.TimeoutExpired:
        engine.kill()
    shutil.rmtree(runtime, ignore_errors=True)
sys.exit(1 if failed else 0)
