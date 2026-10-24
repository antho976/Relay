"""A stand-in for Relay's video worker (plugins/video/runtime/worker.mjs) in the engine's tests.

It speaks the same JSON lines on stdin and stdout and answers as Remotion would for a video with
two compositions: `main` (1080x1920, 30 fps, 3 s) and `wide` (1920x1080). A file in the video's
`src/` changes what it does: `BROKEN` fails the build, `THROWS` throws on every frame, `SLOW`
makes a render run until it is cancelled, `BUNDLING` makes a render bundle for a second first, when
a stop does not reach it (as the worker before 2026-10-24 behaved), `SOUND` gives `main` a sound track: a 120 BPM grid, music
that does not duck, a voice line from 1 s to 2 s and a tap cue at frame 20, five frames off the beat.
"""
import json
import os
import struct
import sys
import threading
import time
import zlib

out_lock = threading.Lock()
cancelled = set()
# The renders past bundling: the only ones a stop reaches.
known = set()

COMPOSITIONS = {
    "main": {"id": "main", "width": 1080, "height": 1920, "fps": 30, "duration_frames": 90, "default_props": {"title": "Hello"},
             "props_schema": {"type": "object", "properties": {"title": {"type": "string", "description": "Headline"}}, "additionalProperties": False}},
    "wide": {"id": "wide", "width": 1920, "height": 1080, "fps": 30, "duration_frames": 60, "default_props": {}},
}


def send(message):
    with out_lock:
        sys.stdout.write(json.dumps(message) + "\n")
        sys.stdout.flush()


def png(path, w, h, rgb):
    raw = b"".join(b"\x00" + bytes(rgb) * w for _ in range(h))

    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
                + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))


def flag(d, name):
    return os.path.exists(os.path.join(d, "src", name))


class Refusal(Exception):
    def __init__(self, code, message):
        super().__init__(message)
        self.code = code


def composition(name):
    if name not in COMPOSITIONS:
        raise Refusal("video.composition", "Could not find composition with ID %s. The following compositions are available: main, wide" % name)
    c = dict(COMPOSITIONS[name])
    c.pop("props_schema", None)
    return c


def check(a):
    # As Remotion prints the page's console: a probe line, which the log leaves out, and a word.
    sys.stderr.write("__relay_probe__{\"frame\": 0}\n" + "checking " + a["dir"] + "\n")
    sys.stderr.flush()
    if flag(a["dir"], "BROKEN"):
        return {"errors": [{"kind": "syntax", "file": "src/Video.tsx", "line": 3, "col": 7, "message": "Expected \"}\" but found \"<\""}], "compositions": []}
    errors = [{"kind": "type", "file": "src/Video.tsx", "line": 9, "col": 3, "message": "TS2322: Type 'string' is not assignable to type 'number'."}] if flag(a["dir"], "TYPED") else []
    return {"errors": errors, "compositions": list(COMPOSITIONS.values())}


def meta(a):
    if flag(a["dir"], "BROKEN"):
        return {"errors": [{"kind": "syntax", "file": "src/Video.tsx", "line": 3, "col": 7, "message": "Expected \"}\""}]}
    return {"composition": composition(a["composition"]), "errors": []}


def still(a):
    c = composition(a["composition"])
    scale = a["scale"]
    stills, errors = [], []
    for frame, output in zip(a["frames"], a["outputs"]):
        if flag(a["dir"], "THROWS"):
            errors.append({"kind": "runtime", "file": "src/Video.tsx", "line": 12, "col": 5, "message": "boom", "frame": frame})
            stills.append({"frame": frame, "path": None, "probe": None, "console": []})
            continue
        png(output, max(1, round(c["width"] * scale)), max(1, round(c["height"] * scale)), (16, 18, 24))
        # A title that runs into the right edge's safe margin on every frame.
        probe = {"frame": frame, "width": c["width"], "height": c["height"],
                 "boxes": [{"id": "title", "x": 700, "y": 800, "w": 340, "h": 90, "opacity": 1}],
                 "texts": [{"id": "title", "path": "div", "text": a.get("props", {}).get("title", "Hello"), "x": 700, "y": 800, "w": 340, "h": 90,
                            "size": 64, "weight": "800", "family": "Inter", "resolved": True, "color": "rgb(245, 247, 250)",
                            "opacity": 1, "clip": None, "within": []}],
                 "images": []}
        stills.append({"frame": frame, "path": output, "probe": probe, "console": []})
    return {"composition": c, "stills": stills, "errors": errors}


def scan(a):
    c = composition(a["composition"])
    first, last = a.get("range") or [0, c["duration_frames"] - 1]
    probes = []
    for frame in range(first, last + 1, a.get("every", 3)):
        texts = []
        # A title rising into place over its first 9 frames, then resting past the right edge's margin.
        y = 1900 - frame * 120 if frame < 9 else 800
        texts.append({"id": "title", "path": "div", "text": "Hello there", "x": 700, "y": y, "w": 340, "h": 90, "size": 64,
                      "family": "Inter", "resolved": True, "color": "rgb(245, 247, 250)", "opacity": 1, "clip": None, "within": []})
        # Six words for a tenth of a second.
        if 30 <= frame <= 33:
            texts.append({"id": "flash", "path": "p", "text": "Six words gone before you read", "x": 100, "y": 1200, "w": 600, "h": 60,
                          "size": 48, "family": "Inter", "resolved": True, "color": "rgb(255, 255, 255)", "opacity": 1, "clip": None, "within": []})
        probe = {"frame": frame, "width": c["width"], "height": c["height"], "boxes": [], "texts": texts, "images": []}
        if flag(a["dir"], "SOUND"):
            probe.update(light_probe(frame, c["width"], c["height"], frame == first))
        probes.append(probe)
    return {"probes": probes, "sounds": [], "console": [], "errors": []}


GRID = {"bpm": 120, "firstBeat": 0, "beatsPerBar": 4, "fps": 30}
SEQUENCES = [{"id": "s1", "parent": None, "type": "sequence", "name": "Hook", "from": 0, "duration": 45, "hidden": False},
             {"id": "s2", "parent": None, "type": "sequence", "name": "Feature", "from": 45, "duration": 45, "hidden": False},
             {"id": "s3", "parent": "s2", "type": "sequence", "name": "Card", "from": 10, "duration": 20, "hidden": False}]


def sound_marks(frame):
    marks = [{"role": "music", "src": "/static-abc/assets/music.mp3", "name": "music", "elapsed": frame, "volume": 0.45}]
    if 30 <= frame < 60:
        marks.append({"role": "voice", "src": "/static-abc/assets/vo-1.mp3", "name": "vo-1", "elapsed": frame - 30, "volume": 1, "frames": 30})
    if 20 <= frame < 26:
        marks.append({"role": "sfx", "src": "/static-abc/assets/sfx-tap.wav", "name": "tap", "elapsed": frame - 20, "volume": 0.6, "frames": 6})
    return marks


def light_probe(frame, w, h, first):
    probe = {"frame": frame, "width": w, "height": h, "sounds": sound_marks(frame), "grid": GRID}
    if first:
        probe["sequences"] = SEQUENCES
    return probe


def wav(path, seconds, peak):
    import math
    rate, n = 48000, int(48000 * seconds)
    frames = bytearray()
    for i in range(n):
        v = int(peak * 32767 * math.sin(2 * math.pi * 440 * i / rate))
        frames += struct.pack("<hh", v, v)
    with open(path, "wb") as f:
        f.write(b"RIFF" + struct.pack("<I", 36 + len(frames)) + b"WAVEfmt " + struct.pack("<IHHIIHH", 16, 1, 2, rate, rate * 4, 4, 16)
                + b"data" + struct.pack("<I", len(frames)) + bytes(frames))


def timeline(a):
    c = composition(a["composition"])
    sound = flag(a["dir"], "SOUND")
    probes = [light_probe(f, c["width"], c["height"], f == 0) if sound else {"frame": f, "width": c["width"], "height": c["height"]}
              for f in range(c["duration_frames"])]
    mix = None
    if sound and a.get("mix"):
        wav(a["mix"], c["duration_frames"] / c["fps"], 0.3)
        mix = a["mix"]
    return {"composition": c, "probes": probes, "sounds": [], "mix": mix, "console": [], "errors": []}


def render(a):
    job = a["job"]
    if flag(a["dir"], "BUNDLING"):
        time.sleep(1.0)
    known.add(job)
    slow = flag(a["dir"], "SLOW")
    steps = [0.05, 0.15, 0.35, 0.55, 0.75, 0.95]
    for p in steps:
        if job in cancelled:
            raise Refusal("video.render.cancelled", "the render was stopped")
        send({"event": "progress", "job": job, "progress": p, "rendered": int(p * 90), "encoded": 0, "stage": "encoding"})
        time.sleep(0.02)
    while slow and job not in cancelled:
        time.sleep(0.02)
    if job in cancelled:
        raise Refusal("video.render.cancelled", "the render was stopped")
    os.makedirs(os.path.dirname(a["out"]), exist_ok=True)
    with open(a["out"], "wb") as f:
        f.write(b"fake " + a["format"].encode() + b" " + json.dumps(a.get("props", {})).encode())
    return {"path": a["out"], "bytes": os.path.getsize(a["out"]), "frames": 90, "ms": 5}


def cancel(a):
    if a["job"] not in known:
        return {"cancelled": False}
    cancelled.add(a["job"])
    return {"cancelled": True}


COMMANDS = {"hello": lambda a: {"node": "fake", "remotion": "4.0.534"}, "check": check, "meta": meta, "still": still, "scan": scan,
            "timeline": timeline, "render": render, "cancel": cancel, "forget": lambda a: {}}


def handle(request):
    rid, cmd = request.pop("id"), request.pop("cmd")
    try:
        send({"id": rid, "ok": True, "result": COMMANDS[cmd](request)})
    except Refusal as e:
        send({"id": rid, "ok": False, "error": {"code": e.code, "message": str(e)}})
    except Exception as e:  # noqa: BLE001 - every failure is an answer, as the real worker's
        send({"id": rid, "ok": False, "error": {"code": "video.worker", "message": repr(e)}})


for line in sys.stdin:
    if line.strip():
        threading.Thread(target=handle, args=(json.loads(line),), daemon=True).start()
