#!/usr/bin/env python3
"""The Design view against a disposable engine: fixture designs written through design.apply as
an agent would, then relay-native captures and scripted runs of the board (design/drive.rs).

    python3 scripts/design-smoke.py [shot ...]

Shots (default all):
- gallery, board, board-select, board-narrow (the text toolbar in a 1080 px window);
- edit: typing in place, the edit view on screen with the layers panel shut (E-board-25);
- script: gestures and commands; the client's scene must equal the engine's;
- agent: Ask types into the fixture terminal, whose fake agent edits the layer; the open board
  must show it, and Undo agent put it back;
- agent-undo: an agent recolours a layer the person just edited; Ctrl+Z undoes only the
  person's edit, keeping the agent's colour, and Undo <agent> reverts it (TM-10);
- agent-stale: the person's text edit lands after an agent changed the same text: "Not saved",
  the field named, the newer version shown;
- agent-delete: an agent deletes one layer, then three: the status names the agent, the toast
  counts them, and Undo agent restores them (TM-12);
- media: a picture through design.asset.add, copy and paste;
- shapes: polygons, icons, gradients, dashes, rotation, connectors and labels as an agent writes
  them, then the tools that make them, and an unknown icon refused;
- more: arcs, rings, glows, closed and smooth paths, flips, blends, inner shadows, text runs and
  case as an agent writes them, `radius` on an ellipse refused, then the vector pen, flip, copy
  and paste style; the PNG, SVG and PDF exports, the vector ones rasterised and compared with
  the PNG (I-12, TM-21), and a shape's shadow as soft as a box's (TM-25);
- reuse: named colours, styles and a copy; a colour changed from outside is re-read and redrawn;
- motion: Play, pause, seek, a contact sheet and an APNG; the design's rev stays as it was;
- render: design.render answered by the open app: sizes, boxes, issues, its traces;
- glass: a tab bar frosting what scrolls under it (backdrop_blur) with a top border alone
  (stroke_widths), a pill taking both from a style, a chart with one bar of its own colour, and a
  fade sliding up in a hugging column, which design.render's motion checks pass over; then the
  inspector's Border and Layer sections for the tab bar;
- pictures: the image key's popover searching the picture fixtures, a photo placed with its
  credit and the inspector's Credit section;
- perf: a 1,721-node design: layout, content and frame times.

Each board opens its design by id (RELAY_SMOKE_DESIGN_ID), not as the gallery's first card.
Needs target/debug/relay and target/debug/relay-native (cargo build -p relay-cli -p
relay-native), Pillow, and kwin_wayland: the script starts its own virtual KWin
(`kwin_wayland --virtual --no-lockscreen`) and opens every window there, never on someone's
desktop. rsvg-convert and pdftoppm, where installed, rasterise the vector exports. Captures,
logs and readings (design-readings.json) go to .impeccable/review/design-*. Exits non-zero when
a shot fails."""
import json, os, re, socket, statistics, struct, subprocess, sys, tempfile, time, traceback, uuid, zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / ".impeccable/review"; OUT.mkdir(parents=True, exist_ok=True)
ALL = ["gallery", "board", "board-select", "board-narrow", "edit", "script", "agent", "agent-undo", "agent-stale",
       "agent-delete", "media", "shapes", "more", "reuse", "motion", "render", "glass", "pictures", "perf"]
SHOTS = sys.argv[1:] or ALL
unknown = [s for s in SHOTS if s not in ALL + ["gallery-narrow"]]
if unknown:
    sys.exit(f"unknown shots {unknown}; known: {' '.join(ALL)}")
FIXTURES = ROOT / "crates/relay-core/tests/fixtures/pictures"
FAKE_AGENT = r'''#!/usr/bin/env python3
"""A fixture provider: echoes input, and answers a [Relay design] prompt by turning the
selected layer green through design.apply, as the session's own agent. It says it is at its prompt
whenever it is, as a real agent's hooks do, and leaves its session and token where the harness
can act as it."""
import json, os, re, subprocess, sys
if "--version" in sys.argv:
    print("fixture 1.0"); sys.exit()
if "auth" in sys.argv or "login" in sys.argv:
    print('{"loggedIn":true}'); sys.exit()
smoke = os.environ.get("RELAY_SMOKE_DIR")
if smoke and os.environ.get("RELAY_TOKEN"):
    with open(os.path.join(smoke, "agent.json.part"), "w") as f:
        json.dump({"session": os.environ.get("RELAY_SESSION", ""), "token": os.environ["RELAY_TOKEN"]}, f)
    os.replace(os.path.join(smoke, "agent.json.part"), os.path.join(smoke, "agent.json"))
print("Fixture agent ready.", flush=True)
def idle():
    """At its prompt again, as a real agent's Stop hook says: Ask types only into an idle agent."""
    subprocess.run([os.environ["RELAY_BIN"], "cmd", "session.report",
                    json.dumps({"session": os.environ.get("RELAY_SESSION", ""), "kind": "idle"})], capture_output=True)
idle()
for line in sys.stdin:
    print("echo: " + line.rstrip()[:400], flush=True)
    m = re.search(r"design_id (\d+)", line)
    sel = re.search(r"Selected: ([A-Za-z0-9_.:-]+)", line)
    if m and sel:
        ops = [{"op": "update", "id": sel.group(1), "set": {"fill": "#2EC469"}}]
        done = subprocess.run([os.environ["RELAY_BIN"], "cmd", "design.apply",
                               json.dumps({"design_id": int(m.group(1)), "ops": ops})], capture_output=True, text=True)
        print("agent applied: " + (done.stdout.strip()[:200] or done.stderr.strip()[:400]), flush=True)
    idle()
'''

ENGINE = ROOT / "target/debug/relay"
NATIVE = ROOT / "target/debug/relay-native"
for binary in (ENGINE, NATIVE):
    if not binary.is_file():
        sys.exit(f"{binary} is missing: cargo build -p relay-cli -p relay-native")


def git(repo, *args):
    subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True)


def text(id, parent, words, **kw):
    n = {"id": id, "parent": parent, "kind": "text", "text": words}
    n.update(kw)
    return {"op": "add", "node": n}


def node(id, parent, kind, **kw):
    n = {"id": id, "parent": parent, "kind": kind}
    n.update(kw)
    return {"op": "add", "node": n}


def settings_screen(page):
    """A mobile settings screen as an agent would write it: auto layout, no measuring."""
    ops = [
        node("phone", page, "frame", name="Settings · iPhone", x=0, y=0, w=390, h=844, fill="#F4F2EE",
             layout="column", gap=20, padding=[56, 20, 24, 20], radius=0),
        text("title", "phone", "Settings", font_size=32, font_weight=700, color="#151413", letter_spacing=-0.6),
        node("profile", "phone", "frame", name="Profile card", fill="#FFFFFF", radius=16, layout="row", gap=14,
             padding=[16], items="center", w_sizing="fill", h_sizing="hug",
             shadow={"x": 0, "y": 2, "blur": 10, "spread": 0, "color": "#0000001A"}),
        node("avatar", "profile", "ellipse", w=52, h=52, fill="#2EC469"),
        node("who", "profile", "frame", name="Name", fill="#00000000", layout="column", gap=2, w_sizing="fill", h_sizing="hug"),
        text("name", "who", "Antho", font_size=17, font_weight=600, color="#151413"),
        text("mail", "who", "4 agents · 2 designs", font_size=13, color="#7A756E"),
        text("section", "phone", "Notifications", font_size=13, font_weight=600, color="#7A756E", letter_spacing=0.2),
        node("list", "phone", "frame", name="Settings list", fill="#FFFFFF", radius=16, layout="column",
             padding=[4, 16], w_sizing="fill", h_sizing="hug"),
    ]
    for i, (label, on) in enumerate([("Agent needs input", True), ("Agent finished", True), ("Guardrail triggered", False), ("Weekly usage report", True)]):
        row = f"row{i}"
        ops += [
            node(row, "list", "frame", name=label, fill="#00000000", layout="row", items="center", justify="space_between",
                 padding=[14, 0], w_sizing="fill", h_sizing="hug"),
            text(f"{row}-label", row, label, font_size=15, color="#151413"),
            node(f"{row}-toggle", row, "frame", name="Toggle", w=46, h=28, radius=14,
                 fill="#2EC469" if on else "#D9D5CF", layout="row", padding=[3], justify="end" if on else "start"),
            node(f"{row}-knob", f"{row}-toggle", "ellipse", w=22, h=22, fill="#FFFFFF"),
        ]
    ops += [
        node("cta", "phone", "frame", name="Button", fill="#151413", radius=14, layout="row", justify="center",
             items="center", padding=[16], w_sizing="fill", h_sizing="hug"),
        text("cta-label", "cta", "Save changes", font_size=16, font_weight=600, color="#FFFFFF"),
        # Beside it: a dark variant and notes.
        node("dark", page, "frame", name="Settings · Dark", x=450, y=0, w=390, h=844, fill="#131211",
             layout="column", gap=20, padding=[56, 20, 24, 20]),
        text("dark-title", "dark", "Settings", font_size=32, font_weight=700, color="#EDE9E2", letter_spacing=-0.6),
        node("dark-card", "dark", "frame", name="Profile card", fill="#1F1D1B", radius=16, layout="row", gap=14,
             padding=[16], items="center", w_sizing="fill", h_sizing="hug"),
        node("dark-avatar", "dark-card", "ellipse", w=52, h=52, fill="#4C9DFF"),
        text("dark-name", "dark-card", "Dark mode keeps the same rhythm", font_size=15, color="#EDE9E2", w_sizing="fill"),
        node("note1", page, "sticky", x=900, y=40, w=220, h=200,
             text="Ask the agent: tighten the list rows and try a 12px radius"),
        node("note2", page, "sticky", x=900, y=270, w=220, h=170, fill="#AEDBFF",
             text="Dark variant: check contrast of the section labels"),
        node("arrow", page, "line", x=880, y=150, w=40, h=60, points=[[40, 0], [0, 60]], stroke="#EDE9E2",
             stroke_width=2, arrow_end=True),
        text("canvas-note", page, "v2 — after the review", x=0, y=-110, font_size=20, font_weight=600),
        node("scribble", page, "path", x=1160, y=60, w=120, h=60, stroke="#FFB020", stroke_width=3,
             points=[[0, 40], [20, 10], [40, 50], [60, 5], [80, 45], [100, 15], [120, 30]]),
    ]
    return ops


def png_file(path, w, h, pixel):
    """A PNG written without any image tool: `pixel(x, y)` gives each (r, g, b)."""
    rows = b"".join(b"\x00" + bytes(c for x in range(w) for c in pixel(x, y)) for y in range(h))
    chunk = lambda kind, data: struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
                     + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b""))


def png_chunks(path):
    """Each chunk's kind, counted, and how many CRCs fail."""
    data = Path(path).read_bytes(); at = 8; bad = 0; kinds = {}
    assert data[:8] == b"\x89PNG\r\n\x1a\n", path
    while at < len(data):
        ln = struct.unpack(">I", data[at:at + 4])[0]; kind = data[at + 4:at + 8].decode()
        if zlib.crc32(data[at + 4:at + 8 + ln]) != struct.unpack(">I", data[at + 8 + ln:at + 12 + ln])[0]:
            bad += 1
        kinds[kind] = kinds.get(kind, 0) + 1; at += 12 + ln
    return kinds, bad


def image():
    """Pillow, for the shots that read pixels."""
    try:
        from PIL import Image, ImageChops, ImageStat
    except ImportError:
        sys.exit("these shots read pixels with Pillow: pip install pillow (or the distribution's python-pillow)")
    return Image, ImageChops, ImageStat


def on_white(img):
    Image, _, _ = image()
    img = img.convert("RGBA")
    return Image.alpha_composite(Image.new("RGBA", img.size, (255, 255, 255, 255)), img).convert("RGB")


def mean_diff(a, b):
    """Mean absolute difference of two same-size images over white, 0..255."""
    _, ImageChops, ImageStat = image()
    return sum(ImageStat.Stat(ImageChops.difference(on_white(a), on_white(b))).mean) / 3


def colour_box(img, rgb, tol=6):
    """The box of the pixels within `tol` of `rgb`, or None."""
    img = on_white(img)
    w = img.size[0]
    raw = img.tobytes()
    hits = [i // 3 for i in range(0, len(raw), 3)
            if abs(raw[i] - rgb[0]) <= tol and abs(raw[i + 1] - rgb[1]) <= tol and abs(raw[i + 2] - rgb[2]) <= tol]
    if not hits:
        return None
    xs = [i % w for i in hits]; ys = [i // w for i in hits]
    return min(xs), min(ys), max(xs), max(ys)


def shadow_band(img, rgb):
    """How far above a shape's top edge, in px, its shadow on white stays darker than half its
    darkness just outside the edge (the width of its 50% band), up the shape's middle column;
    and that darkness."""
    img = on_white(img)
    box = colour_box(img, rgb)
    assert box, f"no pixels of {rgb}"
    cx = (box[0] + box[2]) // 2
    px = img.load()
    top = next(y for y in range(box[1], box[3] + 1) if all(abs(px[cx, y][k] - rgb[k]) <= 6 for k in range(3)))
    dark = [255 - sum(px[cx, top - k]) / 3 for k in range(2, min(top, 120))]
    assert dark and dark[0] > 20, f"no shadow above {rgb}: {dark[:8]}"
    return next((k for k, d in enumerate(dark) if d < dark[0] / 2), len(dark)) + 2, round(dark[0], 1)


def series(log, kind):
    """The milliseconds of each `DESIGN_TRACE <kind> <ms> ms` line."""
    return [float(l.split()[2]) for l in log.splitlines() if l.startswith(f"DESIGN_TRACE {kind} ")]


def summary(v):
    v = sorted(v)
    if not v:
        return None
    return {"n": len(v), "median_ms": round(statistics.median(v), 3),
            "p95_ms": round(v[min(len(v) - 1, round(0.95 * (len(v) - 1)))], 3), "max_ms": round(v[-1], 3)}


def lines(log, tag):
    """The JSON after each `tag ` line of a native log."""
    return [json.loads(l.split(" ", 1)[1]) for l in log.splitlines() if l.startswith(tag + " ")]


def by_id(nodes):
    return {n["id"]: n for n in nodes}


class Run:
    """A finished relay-native run: its log, and what its script printed, in order."""

    def __init__(self, log):
        self.log = log
        self.scenes = [by_id(s) for s in lines(log, "DESIGN_SCENE")]
        self.chrome = lines(log, "DESIGN_CHROME")
        self.styles = lines(log, "DESIGN_STYLES")


with tempfile.TemporaryDirectory(prefix="relay-design-shot-") as temporary:
    base = Path(temporary)
    runtime = base / "runtime"; runtime.mkdir(mode=0o700)
    env = dict(os.environ, XDG_DATA_HOME=str(base / "data"), XDG_CONFIG_HOME=str(base / "config"),
               XDG_CACHE_HOME=str(base / "cache"), XDG_RUNTIME_DIR=str(runtime), RELAY_INSTANCE="test",
               RELAY_BIN=str(ENGINE), NO_COLOR="1", RELAY_SMOKE_DIR=str(base), RELAY_PICTURE_FIXTURES=str(FIXTURES))
    for key in ("RELAY_SESSION", "RELAY_TOKEN", "RELAY_BRIEF", "WAYLAND_DISPLAY"):
        env.pop(key, None)
    provider = base / "provider"
    provider.write_text(FAKE_AGENT)
    provider.chmod(0o755)
    log = (OUT / "engine.log").open("w")
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
                    raise RuntimeError("engine failed; see engine.log")
                time.sleep(0.05)
        stream = conn.makefile("rwb")

        def call(op, payload, ok=True, actor="user", token=None):
            """One request: its result, or with `ok=False` its error when refused."""
            ask = dict(v=1, id=str(uuid.uuid4()), actor=actor, op=op, payload=payload)
            if token:
                ask["token"] = token
            stream.write((json.dumps(ask) + "\n").encode())
            stream.flush()
            while True:
                r = json.loads(stream.readline())
                if "ok" in r:
                    break
            if ok and not r["ok"]:
                raise RuntimeError(f"{op}: {r['error']}")
            return r["result"] if r["ok"] else r["error"]

        def agent():
            """The fixture terminal's agent: its session name and token, once it has started."""
            deadline = time.monotonic() + 15
            while not (base / "agent.json").is_file():
                assert time.monotonic() < deadline, "the fixture agent never started"
                time.sleep(0.1)
            return json.loads((base / "agent.json").read_text())

        def as_agent(op, payload):
            """A request as the fixture terminal's agent would send it."""
            who = agent()
            return call(op, payload, actor="agent:" + who["session"], token=who["token"])

        ws_dir = base / "workspace"; repo = ws_dir / "RelayV4"; repo.mkdir(parents=True)
        git(repo, "init", "-q", "-b", "main"); git(repo, "config", "user.name", "f"); git(repo, "config", "user.email", "f@x")
        (repo / "README.md").write_text("# fixture\n"); git(repo, "add", "."); git(repo, "commit", "-qm", "fixture")
        ws = call("workspace.create", {"path": str(ws_dir)})
        project = call("project.add", {"workspace_id": ws["id"], "path": str(repo)})
        pid = project["id"]
        call("settings.set", {"path": "providers.claude.path", "value": str(provider)})
        session = call("session.create", {"project_id": pid, "provider": "claude", "role": "builder"})
        sname = session["name"] if "name" in session else session["session"]["name"]
        call("session.spawn", {"session": sname})

        timings = {}
        # Each shot's design, by name: the board opens it by id.
        designs = {}

        def create(title, ops=None, **kw):
            """A design written as an agent writes one: its id and its first page."""
            d = call("design.create", dict(project_id=pid, title=title, **kw))
            page = d["nodes"][0]["id"]
            if ops:
                call("design.apply", {"design_id": d["design"]["id"], "ops": ops(page)})
            return d["design"]["id"], page

        create("Onboarding flow", lambda p2: [
            node("s1", p2, "frame", name="Welcome", x=0, y=0, w=360, h=640, fill="#FFFFFF", layout="column", padding=[40, 24], gap=12),
            text("s1t", "s1", "Welcome to Relay", font_size=26, font_weight=700),
            text("s1b", "s1", "Agents, terminals and designs in one place.", font_size=15, color="#666666", w_sizing="fill"),
            node("s2", p2, "frame", name="Connect", x=420, y=0, w=360, h=640, fill="#FFFFFF"),
            node("a1", p2, "line", x=370, y=300, w=40, h=0, points=[[0, 0], [40, 0]], stroke="#EDE9E2", stroke_width=2, arrow_end=True),
        ])
        designs["settings"], page1 = create("Settings redesign", session=sname)
        t0 = time.monotonic()
        out = call("design.apply", {"design_id": designs["settings"], "ops": settings_screen(page1)})
        timings["apply_settings_ms"] = round((time.monotonic() - t0) * 1000, 2)
        timings["settings_nodes"] = out["design"]["nodes"]
        call("design.delete", {"design_id": create("Old logo ideas")[0]})
        # The shots that leave their edits behind each edit a copy of the settings screen of their
        # own, tied to the fixture terminal, so no other shot sees them.
        for name in ("edit", "agent-undo", "agent-stale", "agent-delete"):
            if name in SHOTS:
                designs[name] = create(f"Settings ({name})", settings_screen, session=sname)[0]
        if "shapes" in SHOTS:
            designs["shapes"], sp = create("Shapes")
            call("design.apply", {"design_id": designs["shapes"], "ops": [
                node("tri", sp, "polygon", x=0, y=0, w=160, h=140, sides=3, fill="#FFB020"),
                node("star", sp, "polygon", x=200, y=0, w=150, h=150, sides=5, star=0.42, fill="#FFD60A", stroke="#1A1A1A", stroke_width=2),
                node("hex", sp, "polygon", x=390, y=0, w=150, h=140, sides=6,
                     gradient={"kind": "linear", "angle": 135, "stops": [{"at": 0, "color": "#4C9DFF"}, {"at": 1, "color": "#A178FF"}]}),
                node("card", sp, "rect", x=0, y=220, w=220, h=120, radii=[24, 4, 24, 4], text="Start here", font_size=18, font_weight=600,
                     gradient={"kind": "radial", "stops": [{"at": 0, "color": "#2EC469"}, {"at": 1, "color": "#0E6B35"}]}),
                node("goal", sp, "ellipse", x=420, y=230, w=180, h=110, fill="#1F1D1B", stroke="#EDE9E2", stroke_width=2, dash=[6, 4], text="Ship it"),
                node("link", sp, "line", **{"from": "card", "to": "goal"}, route="elbow", arrow_end=True, stroke="#EDE9E2", stroke_width=2),
                node("tilt", sp, "rect", x=650, y=40, w=160, h=60, fill="#FF5C5C", radius=12, rotation=-12, text="Turned", font_weight=600),
                node("soft", sp, "ellipse", x=660, y=220, w=120, h=120, fill="#A178FF", blur=8),
                node("i1", sp, "icon", x=0, y=400, w=32, h=32, icon="settings"),
                node("i2", sp, "icon", x=50, y=400, w=32, h=32, icon="bell", stroke="#FFB020"),
                node("i3", sp, "icon", x=100, y=400, w=32, h=32, icon="heart", stroke="#FF5C5C"),
                node("i4", sp, "icon", x=150, y=400, w=32, h=32, icon="search"),
                node("i5", sp, "icon", x=200, y=400, w=32, h=32, icon="calendar", stroke="#2EC469"),
                node("i6", sp, "icon", x=250, y=400, w=32, h=32, icon="bar-chart", stroke="#4C9DFF"),
                node("curve", sp, "line", **{"from": "tri", "to": "tilt"}, route="curve", arrow_end=True, stroke="#8C877F", stroke_width=2, dash=[1, 5]),
            ]})
            refused = call("design.apply", {"design_id": designs["shapes"], "ops": [node("bad", sp, "icon", icon="no-such")]}, ok=False)
            assert "is not one of Relay" in refused.get("message", ""), ("an unknown icon must be refused", refused)
        if "more" in SHOTS:
            designs["more"], mp = create("More shapes")
            # The same shadow under a box and under part of an ellipse, on white (TM-25).
            shade = {"x": 0, "y": 0, "blur": 16, "spread": 0, "color": "#000000"}
            call("design.apply", {"design_id": designs["more"], "ops": [
                node("donut", mp, "ellipse", x=0, y=0, w=160, h=160, fill="#4C9DFF", arc={"start": 0, "sweep": 270, "inner": 0.62}),
                node("pie", mp, "ellipse", x=200, y=0, w=160, h=160, fill="#FFB020", arc={"start": 30, "sweep": 120}),
                node("gauge", mp, "ellipse", x=400, y=0, w=160, h=160, fill="#2EC469", arc={"start": 270, "sweep": 180, "inner": 0.7}),
                node("blob", mp, "line", x=600, y=10, w=160, h=140, closed=True, smooth=True, fill="#A178FF", stroke="#EDE9E2", stroke_width=2,
                     points=[[0, 70], [60, 0], [160, 40], [120, 140], [30, 120]]),
                node("tri", mp, "polygon", x=0, y=220, w=140, h=120, sides=3, fill="#FF5C5C", flip_y=True),
                node("c1", mp, "ellipse", x=200, y=210, w=120, h=120, fill="#FFD60A"),
                node("c2", mp, "ellipse", x=260, y=240, w=120, h=120, fill="#4C9DFF", blend="multiply"),
                node("card", mp, "rect", x=430, y=220, w=200, h=110, fill="#2A2826", radius=18,
                     shadow={"x": 0, "y": 3, "blur": 10, "spread": 0, "color": "#00000099", "inset": True}),
                text("label", mp, "section heading", x=0, y=380, font_size=13, font_weight=600, letter_spacing=1.2, text_case="upper", color="#8C877F"),
                text("tight", mp, "Tight\nheading", x=0, y=410, font_size=40, font_weight=700, line_height=1.0, color="#EDE9E2"),
                node("dashes", mp, "line", x=240, y=430, w=300, h=0, points=[[0, 0], [300, 0]], stroke="#EDE9E2", stroke_width=6, dash=[18, 12]),
                # feature-drawing's list: a progress ring, a glowing line, a dashed polygon, a
                # run, one line of a long sentence and a label with padding.
                node("ring", mp, "ellipse", x=0, y=540, w=140, h=140, arc={"start": 0, "sweep": 250, "inner": 0.78, "cap": "round"},
                     gradient={"kind": "conic", "stops": [{"at": 0, "color": "#4C9DFF"}, {"at": 1, "color": "#2EC469"}]}),
                node("glow", mp, "line", x=180, y=600, w=220, h=0, points=[[0, 0], [220, 0]], stroke_width=6, stroke_cap="round",
                     stroke_gradient={"kind": "linear", "stops": [{"at": 0, "color": "#FF5C5C"}, {"at": 1, "color": "#FFD60A"}]},
                     shadows=[{"blur": 12, "color": "#FF5C5CCC"}, {"blur": 4, "color": "#FFD60A99"}]),
                node("dashpoly", mp, "polygon", x=440, y=540, w=140, h=130, sides=5, stroke="#EDE9E2", stroke_width=4,
                     dash=[10, 6], stroke_join="miter"),
                text("km", mp, "8.4 km", x=620, y=540, font_size=40, font_weight=700, color="#EDE9E2",
                     runs=[{"start": 4, "end": 6, "color": "#8C877F", "font_size": 20}]),
                text("oneline", mp, "A long sentence that will not fit on one line of a hundred and sixty pixels", x=620, y=610,
                     w=160, font_size=15, max_lines=1, color="#EDE9E2"),
                node("padded", mp, "rect", x=820, y=560, w=200, h=60, fill="#2A2826", radius=8, text="Padded label", font_size=15,
                     color="#EDE9E2", padding=[8, 24]),
                node("shadebox", mp, "frame", name="Shadows", x=0, y=760, w=1500, h=400, fill="#FFFFFF"),
                node("sh-rect", "shadebox", "rect", x=60, y=120, w=600, h=200, fill="#0B7A75", shadow=shade),
                node("sh-arc", "shadebox", "ellipse", x=780, y=120, w=600, h=200, fill="#7A0B5C", arc={"start": -60, "sweep": 120}, shadow=shade),
            ]})
            refused = call("design.apply", {"design_id": designs["more"], "ops": [node("round", mp, "ellipse", w=40, h=40, radius=4)]}, ok=False)
            assert refused.get("code") == "design.node_invalid" and "radius: ellipse nodes do not use it" in refused.get("message", ""), refused
        if "reuse" in SHOTS:
            designs["reuse"] = create("Reuse", lambda rp: [
                {"op": "color", "name": "brand", "value": "#4C9DFF"},
                {"op": "color", "name": "ink", "value": "#151413"},
                {"op": "style", "name": "card", "set": {"fill": "$brand", "radius": 16,
                                                       "shadow": {"x": 0, "y": 4, "blur": 12, "spread": 0, "color": "#00000033"}}},
                {"op": "style", "name": "heading", "set": {"font_size": 22, "font_weight": 700, "color": "$ink"}},
                node("sheet", rp, "frame", name="Sheet", x=0, y=0, w=760, h=360, fill="#FFFFFF"),
                node("plan", "sheet", "frame", name="Plan", x=40, y=40, w=300, h=260, style="card", layout="column", padding=[24], gap=8),
                text("plan-title", "plan", "Pro plan", style="heading"),
                text("plan-price", "plan", "12 a month", font_size=15, color="$ink"),
                {"op": "copy", "id": "plan", "as": "plan-2", "dx": 380},
                {"op": "update", "id": "plan-2-title", "set": {"text": "Team plan"}},
            ])[0]
        if "motion" in SHOTS:
            designs["motion"] = create("Motion", lambda page: [
                node("art", page, "frame", x=0, y=0, w=480, h=320, fill="#FFFFFF", timeline={"duration_ms": 3000, "loop": True}),
                text("title", "art", "Hello motion", x=32, y=32, font_size=32, color="#1A1A1A",
                     motion={"keys": [{"at_ms": 0, "dy": 24, "opacity": 0}, {"at_ms": 500, "dy": 0, "opacity": 1}], "ease": "ease_out"}),
                node("dot", "art", "ellipse", x=40, y=200, w=60, h=60, fill="#E5484D",
                     motion={"keys": [{"at_ms": 0, "dx": 0}, {"at_ms": 1500, "dx": 320}, {"at_ms": 3000, "dx": 0}], "ease": "linear"}),
                node("spin", "art", "icon", x=380, y=40, w=60, h=60, icon="settings", stroke="#30A46C",
                     motion={"keys": [{"at_ms": 0, "rotate": 0}, {"at_ms": 1000, "rotate": 360}], "ease": "linear", "loop": True}),
                node("pop", "art", "rect", x=300, y=200, w=140, h=48, radius=12, fill="#151413", text="Pop", color="#FFFFFF",
                     motion={"keys": [{"at_ms": 300, "scale": 0}, {"at_ms": 900, "scale": 1}], "ease": "overshoot"}),
                node("ring", "art", "ellipse", x=200, y=110, w=80, h=80, fill="#4C9DFF", arc={"start": 0, "sweep": 270, "inner": 0.7, "cap": "round"},
                     motion={"keys": [{"at_ms": 0, "sweep": 0}, {"at_ms": 1500, "sweep": 270}], "ease": "ease_in_out"}),
                node("drawn", "art", "line", x=32, y=290, w=400, h=0, points=[[0, 0], [400, 0]], stroke="#8E4EC6", stroke_width=4,
                     motion={"keys": [{"at_ms": 0, "trim": 0}, {"at_ms": 2000, "trim": 1}]}),
                # A loose layer that slides past the art board.
                node("flyer", page, "rect", x=520, y=40, w=40, h=40, fill="#8E4EC6",
                     motion={"keys": [{"at_ms": 0, "dx": 0}, {"at_ms": 3000, "dx": 400}], "ease": "linear"}),
            ])[0]
        if "render" in SHOTS:
            designs["render"] = create("Render check", lambda page: [
                node("card", page, "frame", x=0, y=0, w=300, h=200, fill="#FFFFFF", layout="column", padding=[16], gap=8),
                text("title", "card", "Hello render", font_size=24),
                node("wide", "card", "rect", w=400, h=40, fill="#4C9DFF"),
            ])[0]
        if "glass" in SHOTS:
            designs["glass"] = create("Glass", lambda page: [
                {"op": "style", "name": "glass", "set": {"fill": "#FFFFFF14", "backdrop_blur": 12, "stroke": "#FF0000", "stroke_widths": [2, 0, 0, 0]}},
                node("screen", page, "frame", x=0, y=0, w=390, h=420, fill="#101418", timeline={"duration_ms": 600, "loop": False}),
                node("trend", "screen", "frame", x=16, y=16, w=358, h=200, fill="#101418", padding=[8],
                     chart={"kind": "bar", "labels": ["M", "T", "W", "T", "F", "S", "S"], "grid": False, "ticks": 0, "color": "#C9D1DC",
                            "series": [{"name": "Minutes", "color": "#3A4A66", "values": [30, 45, 20, 50, 20, 60, 40],
                                        "colors": [None, None, None, None, None, None, "#7FB2FF"]}]}),
                node("today", "screen", "frame", x=16, y=240, w=220, layout="column", h_sizing="hug", padding=[8], fill="#1B2230", clip=False),
                text("today-title", "today", "Strength · Upper body", font_size=16, color="#FFFFFF",
                     motion={"keys": [{"at_ms": 0, "dy": 24, "opacity": 0}, {"at_ms": 600, "dy": 0, "opacity": 1}], "ease": "ease_out"}),
                *[node(f"stripe-{i}", "screen", "rect", x=40 + 8 * i, y=320, w=8, h=100, fill="#FFFFFF" if i % 2 else "#000000") for i in range(16)],
                text("under", "screen", "Strength · Upper body", x=180, y=356, font_size=16, color="#FFFFFF"),
                node("tabbar", "screen", "rect", x=0, y=340, w=390, h=80, fill="#FFFFFF14", backdrop_blur=12, stroke="#FF0000",
                     stroke_widths=[2, 0, 0, 0]),
                *[node(f"bar-{i}", "screen", "rect", x=260 + 8 * i, y=236, w=8, h=72, fill="#FFFFFF" if i % 2 else "#000000") for i in range(14)],
                node("pill", "screen", "rect", x=250, y=260, w=124, h=40, radius=12, style="glass"),
            ])[0]
        if "pictures" in SHOTS:
            designs["pictures"] = create("Pictures", lambda page: [
                node("card", page, "frame", x=0, y=0, w=720, h=480, fill="#FFFFFF"),
                node("row", page, "frame", x=0, y=560, w=720, h=220, fill="#F4F4F5", layout="row", gap=12, padding=[12]),
                node("a", "row", "rect", w=120, h=120, fill="#4C9DFF"),
            ])[0]
            # The engine serves the picture fixtures (RELAY_PICTURE_FIXTURES) once Pictures is on.
            call("plugin.enable", {"plugin_id": "pictures", "project_id": pid, "enabled": True})
        if "perf" in SHOTS or "render" in SHOTS:
            big = call("design.create", {"project_id": pid, "title": "Big design"})
            bp = big["nodes"][0]["id"]
            ops = []
            for a in range(40):
                ab = f"a{a}"
                ops.append(node(ab, bp, "frame", name=f"Screen {a + 1}", x=(a % 8) * 470, y=(a // 8) * 924, w=390, h=844,
                                fill="#F4F2EE", layout="column", gap=12, padding=[48, 20]))
                ops.append(text(f"{ab}-t", ab, f"Screen {a + 1}", font_size=28, font_weight=700))
                ops.append(node(f"{ab}-l", ab, "frame", fill="#FFFFFF", radius=14, layout="column", padding=[4, 14], w_sizing="fill", h_sizing="hug"))
                for r in range(10):
                    row = f"{ab}-r{r}"
                    ops += [node(row, f"{ab}-l", "frame", fill="#00000000", layout="row", items="center", justify="space_between", padding=[12, 0], w_sizing="fill", h_sizing="hug"),
                            text(f"{row}-x", row, f"Setting number {r + 1}", font_size=15),
                            node(f"{row}-s", row, "frame", w=46, h=28, radius=14, fill="#2EC469", layout="row", padding=[3], justify="end"),
                            node(f"{row}-k", f"{row}-s", "ellipse", w=22, h=22, fill="#FFFFFF")]
            t0 = time.monotonic()
            out = call("design.apply", {"design_id": big["design"]["id"], "ops": ops})
            timings["apply_big_ms"] = round((time.monotonic() - t0) * 1000, 2)
            timings["big_nodes"] = out["design"]["nodes"]
            designs["perf"] = big["design"]["id"]
            gets = []
            for _ in range(7):
                t0 = time.monotonic()
                call("design.get", {"design_id": designs["perf"]})
                gets.append((time.monotonic() - t0) * 1000)
            gets.sort()
            timings["get_big_ms_median"] = round(gets[3], 2)
            t0 = time.monotonic()
            call("design.apply", {"design_id": designs["perf"], "ops": [{"op": "update", "id": "a3-r4-x", "set": {"text": "Changed"}}]})
            timings["apply_one_update_ms"] = round((time.monotonic() - t0) * 1000, 2)
        (OUT / "timings.json").write_text(json.dumps(timings, indent=2))
        print(json.dumps(timings))
        readings = {}

        # A 640x400 picture for the media shot: a gradient PNG.
        hero = base / "hero.png"
        png_file(hero, 640, 400, lambda x, y: (76 + x * 80 // 640, 157 - y * 40 // 400, 255))

        # A private virtual compositor: every window opens there, none on a desktop.
        sock = f"relay-design-smoke-{os.getpid()}"
        kwin = subprocess.Popen(["kwin_wayland", "--virtual", "--no-lockscreen", f"--socket={sock}", "--width", "1440", "--height", "900"],
                                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        real_runtime = Path(os.environ.get("XDG_RUNTIME_DIR", f"/run/user/{os.getuid()}"))
        deadline = time.monotonic() + 20
        while not (real_runtime / sock).exists():
            assert kwin.poll() is None and time.monotonic() < deadline, "kwin_wayland --virtual did not start"
            time.sleep(0.05)
        native_env = dict(os.environ, RELAY_NATIVE_SOCKET=str(path), RELAY_INSTANCE="test", XDG_CACHE_HOME=str(base / "native-cache"),
                          WAYLAND_DISPLAY=sock, DISPLAY="", GDK_BACKEND="wayland")
        for key in ("RELAY_SESSION", "RELAY_TOKEN", "RELAY_BRIEF"):
            native_env.pop(key, None)

        def native(shot, design="settings", script=None, seconds=7, during=None, board=True, **extra):
            """relay-native on the Design page: on `design`'s board, opened by its id (TM-58),
            unless `board` is false; `script` driven (drive.rs); `during(wait_mark)` acting while
            it runs. Captures design-<shot>.png, logs design-<shot>.log."""
            output = OUT / f"design-{shot}.png"; output.unlink(missing_ok=True)
            e = dict(native_env, RELAY_NATIVE_SCREENSHOT=str(output), RELAY_NATIVE_SIZE="1440,900", RELAY_NATIVE_FIXTURE="1",
                     RELAY_NATIVE_PAGE="design", RELAY_NATIVE_SMOKE_SECONDS=str(seconds))
            if board:
                e["RELAY_SMOKE_DESIGN_BOARD"] = "1"
                e["RELAY_SMOKE_DESIGN_ID"] = str(designs[design])
            if script:
                e["RELAY_SMOKE_DESIGN_SCRIPT"] = script if isinstance(script, str) else ";".join(script)
            e.update({k: str(v) for k, v in extra.items()})
            log_path = OUT / f"design-{shot}.log"

            def wait_mark(name, timeout=30):
                """Until the script prints `DESIGN_MARK name`."""
                deadline = time.monotonic() + timeout
                while f"DESIGN_MARK {name}\n" not in log_path.read_text():
                    assert time.monotonic() < deadline and proc.poll() is None, f"the script never reached mark:{name}"
                    time.sleep(0.05)
            with log_path.open("w") as nl:
                proc = subprocess.Popen([str(NATIVE)], env=e, stdout=nl, stderr=nl)
                try:
                    if during:
                        during(wait_mark)
                    proc.wait(timeout=seconds + 45)
                finally:
                    if proc.poll() is None:
                        proc.terminate(); proc.wait(timeout=5)
            logtext = log_path.read_text()
            assert "panicked" not in logtext, logtext[-3000:]
            assert output.is_file() and "Screenshot saved" in logtext, ("no capture", logtext[-2000:])
            if script:
                assert "DESIGN_SCRIPT_DONE" in logtext, logtext[-3000:]
                assert "design script: unknown step" not in logtext, logtext[-2000:]
            print(shot, "->", output)
            return Run(logtext)

        def engine_nodes(design):
            return by_id(call("design.get", {"design_id": designs[design]})["nodes"])

        def same_as_engine(scene, design):
            got = engine_nodes(design)
            assert scene == got, ("the board's scene differs from the engine's", sorted(set(scene) ^ set(got)),
                                  [k for k in scene if k in got and scene[k] != got[k]][:5])

        def shot_simple(shot):
            if shot.startswith("gallery"):
                native(shot, board=False, **({"RELAY_NATIVE_SIZE": "1080,760"} if shot == "gallery-narrow" else {}))
            elif shot == "board":
                native(shot)
            elif shot == "board-select":
                native(shot, RELAY_SMOKE_DESIGN_SELECT="cta-label")
            else:
                # A window under ~1150 px: the text toolbar must keep clear of the inspector.
                native(shot, RELAY_NATIVE_SIZE="1080,760", RELAY_SMOKE_DESIGN_SELECT="cta-label")

        def shot_edit(shot):
            # E-board-25: the title typed into, clear of the panels, and captured while its edit
            # view is on screen.
            run = native(shot, shot, seconds=8, script="select:row1-label;wait:300;zoom:1.25;pan:12,4,30;dbl:40,80;type: and alerts;wait:400;chrome")
            said = run.chrome[-1]
            assert not said["layers_open"], ("the layers panel covers the text", said)
            assert said["editing"] and said["edit_mapped"], ("the edit view is not on screen", said)
            x, y, w, h = said["edit_rect"]
            cw, ch = said["canvas"]
            # The whole edit view on the canvas, below the floating toolbar.
            assert x >= 0 and y >= 60 and x + w <= cw and y + h <= ch, ("the typing is off the canvas", said)
            # Closing the window with the edit open saves the words (TM-39).
            assert engine_nodes(shot)["title"]["text"] == "Settings and alerts", engine_nodes(shot)["title"]

        def shot_more_checks(run, exported):
            got = engine_nodes("more")
            same_as_engine(run.scenes[-1], "more")
            drawn = [n for n in got.values() if n["kind"] == "line" and n["id"] not in ("blob", "dashes", "glow")]
            assert len(drawn) == 1 and drawn[0].get("closed") is True and len(drawn[0]["points"]) == 3, drawn
            assert got["tri"].get("flip_x") is True and got["tri"].get("flip_y") is True, got["tri"]
            assert got["pie"].get("fill") == "#2A2826" and got["pie"]["shadow"].get("inset") is True, got["pie"]
            assert got["pie"].get("arc", {}).get("sweep") == 120, "a pasted style keeps the shape's own arc"
            svg = exported["svg"].read_text()
            assert svg.lstrip().startswith("<?xml") and "<svg" in svg, svg[:200]
            # TM-21: a filter's picture names a picture, never a fragment of the document.
            assert not re.search(r"<feImage[^>]*href=\"#", svg), "an <feImage> references '#'"
            assert exported["pdf"].read_bytes()[:5] == b"%PDF-", "not a PDF"
            Image, _, _ = image()
            png = Image.open(exported["png"])
            w, h = png.size
            # I-12a: the vector exports drawn back to pixels match the PNG export.
            diffs = {}
            if subprocess.run(["sh", "-c", "command -v rsvg-convert"], capture_output=True).returncode == 0:
                subprocess.run(["rsvg-convert", "-w", str(w), "-h", str(h), "-o", str(base / "svg.png"), str(exported["svg"])], check=True)
                diffs["svg"] = mean_diff(png, Image.open(base / "svg.png"))
            if subprocess.run(["sh", "-c", "command -v pdftoppm"], capture_output=True).returncode == 0:
                subprocess.run(["pdftoppm", "-png", "-singlefile", "-scale-to-x", str(w), "-scale-to-y", str(h),
                                str(exported["pdf"]), str(base / "pdf")], check=True)
                diffs["pdf"] = mean_diff(png, Image.open(base / "pdf.png"))
            print("vector exports drawn back against the PNG export, mean difference /255:", {k: round(v, 2) for k, v in diffs.items()})
            for kind, d in diffs.items():
                assert d < 4.0, f"the {kind} export drawn back differs from the PNG export by {d:.2f}/255"
            # TM-25b: part of an ellipse casts as soft a shadow as a box.
            box_band, box_dark = shadow_band(png, (0x0B, 0x7A, 0x75))
            arc_band, arc_dark = shadow_band(png, (0x7A, 0x0B, 0x5C))
            print(f"shadow 50% band: box {box_band} px (darkness {box_dark}), arc {arc_band} px (darkness {arc_dark})")
            assert abs(arc_band - box_band) <= 0.15 * box_band, f"a shape's shadow band {arc_band} px against a box's {box_band} px"
            readings["more"] = {"vector_diff": {k: round(v, 3) for k, v in diffs.items()}, "shadow_band_px": [box_band, arc_band]}
            print("MORE_OK", len(svg), "bytes of SVG")

        def shot_more(shot):
            exported = {k: OUT / f"design-more-export.{k}" for k in ("png", "svg", "pdf")}
            for f in exported.values():
                f.unlink(missing_ok=True)
            run = native(shot, "more", seconds=16, script=[
                "fit", "wait:300",
                "tool:vector", "click:700,240", "click:820,240", "click:760,340", "click:700,240",
                "select:tri", "flip:h",
                "select:card", "copy-style", "select:pie", "paste-style",
                "select:blob", "wait:200",
                f"png:{exported['png']}", f"svg:{exported['svg']}", f"pdf:{exported['pdf']}", "wait:300", "print"])
            shot_more_checks(run, exported)

        def shot_agent_undo(shot):
            # TM-10: the person recolours the button, an agent recolours it again; Ctrl+Z undoes
            # only the person's step (the agent's fill stays), and Undo <agent> reverts the agent's.
            def agent_acts(wait_mark):
                wait_mark("agent")
                as_agent("design.apply", {"design_id": designs[shot], "ops": [{"op": "update", "id": "cta", "set": {"fill": "#2EC469"}}]})
            run = native(shot, shot, seconds=16, during=agent_acts, script=[
                "select:cta", 'set:{"fill":"#FF5C5C","radius":4}', "wait:400", "mark:agent", "wait:2000", "print", "chrome",
                "undo", "wait:1000", "print", "chrome",
                "undo-agent", "wait:1500", "print", "chrome"])
            who = agent()["session"]
            heard, undone, reverted = run.scenes[-3:]
            said_heard, said_undone, said_reverted = run.chrome[-3:]
            assert heard["cta"].get("fill") == "#2EC469" and heard["cta"].get("radius") == 4, heard["cta"]
            assert said_heard["agent_undo"] and who in said_heard["status"], said_heard
            # Ctrl+Z: the person's radius goes back, the agent's fill stays, and the toast says so.
            assert undone["cta"].get("fill") == "#2EC469" and undone["cta"].get("radius") == 14, undone["cta"]
            assert "kept 1 later change" in said_undone["toast"] and "fill" in said_undone["toast"], said_undone
            assert said_undone["agent_undo"], ("Undo agent went with Ctrl+Z", said_undone)
            # Undo <agent>: the person's colour from before the agent's change.
            assert reverted["cta"].get("fill") == "#FF5C5C" and reverted["cta"].get("radius") == 14, reverted["cta"]
            assert said_reverted["status"] == f"Undid {who}'s change", said_reverted
            same_as_engine(reverted, shot)
            print("AGENT_UNDO_OK", who)

        def shot_agent_stale(shot):
            # The person types into the title while an agent changes its words: the edit goes out
            # expecting the words the board showed, is refused as stale, and the newer one shows.
            def agent_acts(wait_mark):
                wait_mark("agent")
                as_agent("design.apply", {"design_id": designs[shot], "ops": [{"op": "update", "id": "title", "set": {"text": "Preferences"}}]})
            run = native(shot, shot, seconds=12, during=agent_acts, script=[
                "fit", "wait:300", "dbl:40,80", "type: v3", "wait:200", "mark:agent", "wait:1500", "endedit", "wait:1500", "chrome", "print"])
            said = run.chrome[-1]
            assert said["toast"].startswith("Not saved:") and "text" in said["toast"] and "changed just before" in said["toast"], said
            assert run.scenes[-1]["title"]["text"] == "Preferences", run.scenes[-1]["title"]
            same_as_engine(run.scenes[-1], shot)
            print("AGENT_STALE_OK", said["toast"])

        def shot_agent_delete(shot):
            # TM-12: an agent deletes a layer, then three: the status names it, the toast counts
            # them, and Undo agent brings them back.
            def agent_acts(wait_mark):
                wait_mark("one")
                as_agent("design.apply", {"design_id": designs[shot], "ops": [{"op": "delete", "id": "note2"}]})
                wait_mark("three")
                as_agent("design.apply", {"design_id": designs[shot], "ops": [{"op": "delete", "id": i} for i in ("note1", "arrow", "scribble")]})
            run = native(shot, shot, seconds=18, during=agent_acts, script=[
                "fit", "wait:300", "mark:one", "wait:2000", "print", "chrome", "undo-agent", "wait:1500", "print", "chrome",
                "mark:three", "wait:2000", "print", "chrome", "undo-agent", "wait:1500", "print", "chrome"])
            who = agent()["session"]
            one, back, three, all_back = run.scenes[-4:]
            said_one, said_back, said_three, _ = run.chrome[-4:]
            assert "note2" not in one and said_one["agent_undo"] and who in said_one["status"], said_one
            assert said_one["toast"] == f"{who} removed 1 layer", said_one
            assert "note2" in back and said_back["status"] == f"Undid {who}'s change", said_back
            assert not {"note1", "arrow", "scribble"} & set(three) and said_three["toast"] == f"{who} removed 3 layers", said_three
            assert said_three["agent_undo"], said_three
            assert {"note1", "arrow", "scribble", "note2"} <= set(all_back), sorted(all_back)
            same_as_engine(all_back, shot)
            print("AGENT_DELETE_OK", who)

        def shot_reuse(shot):
            # feature-reuse: a colour changed from outside (by an agent) is re-read: the board's
            # colours and scene equal the engine's, and what uses the colour redraws in it.
            before, after = OUT / "design-reuse-before.png", OUT / "design-reuse-after.png"

            def recolour(wait_mark):
                wait_mark("recolour")
                as_agent("design.apply", {"design_id": designs[shot], "ops": [{"op": "color", "name": "brand", "value": "#E5484D"}]})
            run = native(shot, shot, seconds=10, during=recolour, script=[
                "fit", "wait:300", f"png:{before}", "print", "mark:recolour", "wait:1500", f"png:{after}", "print"])
            got = call("design.get", {"design_id": designs[shot]})
            assert run.styles[0]["colors"]["brand"] == "#4C9DFF", run.styles[0]
            assert run.styles[-1]["colors"] == got["colors"] and run.styles[-1]["colors"]["brand"] == "#E5484D", (run.styles[-1], got["colors"])
            assert run.styles[-1].get("styles", {}) == got["styles"], (run.styles[-1], got["styles"])
            same_as_engine(run.scenes[-1], shot)
            assert run.scenes[-1]["plan-2"].get("style") == "card" and run.scenes[-1]["plan-2-title"]["text"] == "Team plan"
            Image, _, _ = image()
            old, new = Image.open(before), Image.open(after)
            assert colour_box(old, (0x4C, 0x9D, 0xFF)) and not colour_box(old, (0xE5, 0x48, 0x4D)), "the first export lacks the brand colour"
            assert colour_box(new, (0xE5, 0x48, 0x4D)) and not colour_box(new, (0x4C, 0x9D, 0xFF)), "the board did not redraw the new brand colour"
            print("REUSE_OK")

        def shot_motion(shot):
            sheet, apng, at1200 = OUT / "design-motion-sheet.png", OUT / "design-motion.apng", OUT / "design-motion-1200.png"
            for f in (sheet, apng, at1200):
                f.unlink(missing_ok=True)
            rev = call("design.get", {"design_id": designs[shot]})["design"]["rev"]
            run = native(shot, shot, seconds=18, RELAY_DESIGN_TRACE=1, script=[
                "fit", "wait:600", "play", "wait:400", "pause", "seek:1200", f"png:{at1200}@1200", f"sheet:6@{sheet}", "stop",
                f"apng:{apng}", "print"])
            # Play never writes.
            assert call("design.get", {"design_id": designs[shot]})["design"]["rev"] == rev, "Play changed the design's rev"
            same_as_engine(run.scenes[-1], shot)
            said = [l for l in run.log.splitlines() if l.startswith(("DESIGN_SHEET", "DESIGN_APNG", "DESIGN_PNG"))]
            assert len(said) == 3 and not any("failed" in l for l in said), said
            Image, _, _ = image()
            cells = Image.open(sheet)
            assert cells.size[0] > cells.size[1], ("a sheet of six is three across and two down", cells.size)
            anim = Image.open(apng)
            frames = getattr(anim, "n_frames", 1)
            kinds, bad = png_chunks(apng)
            print(f"APNG {anim.size}: {frames} frames, chunks {kinds}, bad CRCs {bad}")
            # Three seconds at 30 frames a second.
            assert frames == 90 and kinds.get("acTL") == 1 and kinds.get("fcTL") == 90 and bad == 0, (frames, kinds, bad)
            seen = []
            for i in (0, 22, 45):
                anim.seek(i); seen.append(anim.convert("RGBA").copy())
            assert mean_diff(seen[0], seen[1]) > 0.3 and mean_diff(seen[1], seen[2]) > 0.3, "the APNG's frames do not move"
            play = summary(series(run.log, "play_frame"))
            assert play and play["p95_ms"] < 8.3, play
            m = re.search(r"in (\d+) ms; canvas (\d+) frames, longest ([\d.]+) ms", next(l for l in said if l.startswith("DESIGN_APNG")))
            readings["motion"] = {"play_frame": play, "apng_ms": int(m.group(1)), "apng_canvas_frames": int(m.group(2)),
                                  "apng_longest_frame_ms": float(m.group(3))}
            print("MOTION_OK", json.dumps(readings["motion"]))

        def shot_render(shot):
            import math
            # design.render is drawn by the open app: with none open it is refused at once.
            refused = call("design.render", {"design_id": designs[shot]}, ok=False)
            assert refused.get("code") == "design.render.no_renderer", refused
            took, traced = {}, {}

            def renders(_wait_mark):
                deadline = time.monotonic() + 30
                while True:
                    got = call("design.render", {"design_id": designs[shot], "timeout_ms": 15000}, ok=False)
                    if got.get("code") != "design.render.no_renderer" or time.monotonic() > deadline:
                        break
                    time.sleep(0.2)
                assert got.get("png"), got
                png = Path(got["png"]["path"]).read_bytes()
                w, h = struct.unpack(">II", png[16:24])
                assert png[:8] == b"\x89PNG\r\n\x1a\n" and (w, h) == (math.ceil(got["area"]["w"] * got["scale"]), math.ceil(got["area"]["h"] * got["scale"])), (w, h, got["area"])
                (OUT / "design-render-page.png").write_bytes(png)
                rows = {r[0]: r for r in got["boxes"]["rows"]}
                # Every node but the page, the hugged title measured.
                assert got["boxes"]["total"] == 3 and set(rows) == {"card", "title", "wide"} and rows["title"][5] > 0, got["boxes"]
                assert any(i["code"] in ("overflow", "clipped") and "wide" in i["ids"] for i in got["issues"]), got["issues"]
                nodes = call("design.render", {"design_id": designs[shot], "nodes": ["title"], "scale": 2})
                assert nodes["scale"] == 2 and nodes["png"]["w"] == math.ceil(nodes["area"]["w"] * 2), (nodes["area"], nodes["png"])
                bare = call("design.render", {"design_id": designs[shot], "png": False})
                assert bare.get("png") is None and bare["boxes"]["total"] == got["boxes"]["total"], bare
                # Readings: this design, the settings screen and the 1,721-node one, ten renders each.
                for name in (shot, "settings", "perf"):
                    seen = len((OUT / "design-render.log").read_text())
                    took[name] = [call("design.render", {"design_id": designs[name]})["timings_ms"] for _ in range(10)]
                    # The app's main-thread steps for these ten (render-SPEC §9: each under 8 ms).
                    said = (OUT / "design-render.log").read_text()[seen:]
                    traced[name] = {s: summary(series(said, s)) for s in ("render-boxes", "render-content", "render-layout", "render-paint")}
                big = call("design.render", {"design_id": designs["perf"], "png": False})
                took["perf_boxes"] = big["boxes"]["total"]
                assert big["boxes"]["total"] == timings["big_nodes"] - 1, (big["boxes"]["total"], timings["big_nodes"])
            run = native(shot, shot, seconds=15, during=renders, RELAY_DESIGN_TRACE=1)
            steps = sorted({l.split()[1] for l in run.log.splitlines() if l.startswith("DESIGN_TRACE render-")})
            assert steps, "no render traces"
            readings["render"] = {name: {k: round(statistics.median(t[k] for t in v), 2) for k in ("read", "layout", "paint", "encode")}
                                  for name, v in took.items() if isinstance(v, list)}
            readings["render"]["perf_boxes"] = took["perf_boxes"]
            readings["render"]["traces"] = traced
            print("RENDER_OK", json.dumps(readings["render"]))

        def shot_glass(shot):
            got = {}

            def renders(_wait_mark):
                deadline = time.monotonic() + 30
                while True:
                    r = call("design.render", {"design_id": designs[shot], "nodes": ["screen"], "timeout_ms": 15000}, ok=False)
                    if r.get("code") != "design.render.no_renderer" or time.monotonic() > deadline:
                        break
                    time.sleep(0.2)
                got["still"] = r
                got["frames"] = call("design.render", {"design_id": designs[shot], "nodes": ["screen"], "frames": 6, "png": False})
                got["end"] = call("design.render", {"design_id": designs[shot], "nodes": ["screen"], "frames": 6, "png": False, "motion_checks": "end"})
            native(shot, shot, seconds=12, during=renders)
            Image, _, _ = image()
            still = got["still"]
            assert still.get("png"), still
            picture = Path(still["png"]["path"])
            (OUT / "design-glass-render.png").write_bytes(picture.read_bytes())
            img = on_white(Image.open(picture))
            px = img.load()
            area, scale = still["area"], still["scale"]
            at = lambda x, y: px[int((x - area["x"]) * scale), int((y - area["y"]) * scale)]
            spread = lambda y: statistics.pstdev(sum(at(x, y)) / 3 for x in range(44, 164))
            # The stripes are sharp above the tab bar and frosted under it.
            sharp, frosted = spread(330), spread(390)
            assert sharp > 80 and frosted < sharp / 4, (sharp, frosted)
            # The pill takes its frost and its top border from its style.
            pill = lambda y: statistics.pstdev(sum(at(x, y)) / 3 for x in range(264, 368))
            assert pill(248) > 80 and pill(285) < pill(248) / 4, (pill(248), pill(285))
            assert at(312, 261)[0] > 180 and at(312, 261)[1] < 90 and at(312, 297)[0] < 200, (at(312, 261), at(312, 297))
            # Its border is on its top side alone.
            top, bottom, left = at(300, 341), at(300, 418), at(1, 380)
            assert top[0] > 180 and top[1] < 90, top
            assert bottom[0] < 120 and left[0] < 120, (bottom, left)
            # Today's bar in its own colour, the others in the series'.
            nodes = by_id(call("design.get", {"design_id": designs[shot]})["nodes"])
            centre = lambda b: at(16 + b["x"] + b["w"] / 2, 16 + b["y"] + b["h"] / 2)
            near = lambda c, rgb: all(abs(c[k] - rgb[k]) <= 8 for k in range(3))
            assert near(centre(nodes["trend.bar-0-6"]), (0x7F, 0xB2, 0xFF)), centre(nodes["trend.bar-0-6"])
            assert near(centre(nodes["trend.bar-0-0"]), (0x3A, 0x4A, 0x66)), centre(nodes["trend.bar-0-0"])
            # The fade sliding up in its hugging column: nothing to flag while it settles, nor where it ends.
            for name in ("frames", "end"):
                moving = [i for i in got[name]["issues"] if i.get("at_ms") is not None]
                assert not moving, (name, moving)
            # The inspector: the bar's widest side as its weight, its sides, and its frost.
            native(shot + "-border", shot, seconds=8, script="fit;wait:400;select:tabbar;wait:300;inspector:Border;wait:400")
            native(shot + "-layer", shot, seconds=8, script="fit;wait:400;select:tabbar;wait:300;inspector:Layer;wait:400")
            print("GLASS_OK", json.dumps({"sharp": round(sharp, 1), "frosted": round(frosted, 1), "top": top, "bottom": bottom,
                                          "pill": [round(pill(248), 1), round(pill(285), 1)]}))

        def shot_pictures(shot):
            # The popover alone, searching the fixtures; then a photo placed, with its credit.
            native(shot + "-popover", shot, seconds=8, RELAY_NATIVE_CAPTURE_POPOVER=1, script="fit;wait:400;pictures:kitchen;wait:3000")
            before = call("design.get", {"design_id": designs[shot]})
            run = native(shot, shot, seconds=15, script="fit;wait:400;pictures:kitchen;wait:800;picture-pick:0;wait:1200;inspector:Credit;wait:500;print")
            got = call("design.get", {"design_id": designs[shot]})
            images = [n for n in got["nodes"] if n["kind"] == "image"]
            assert not before.get("credits"), before.get("credits")
            assert len(images) == 1 and images[0]["src"] in got.get("credits", {}), (images, got.get("credits"))
            assert (Path(got["assets_dir"]) / images[0]["src"]).is_file()
            same_as_engine(run.scenes[-1], shot)
            print("PICTURES_OK", images[0]["src"], got["credits"][images[0]["src"]])

        # The first shots' scripts, by shot: (seconds, design, script).
        scripts = {
            "media": (10, "settings", [
                f"image:{hero}@1200,500", "wait:800",
                "select:profile", "copy", "select:dark", "paste", "wait:400", "print"]),
            "shapes": (14, "shapes", [
                "fit", "wait:300",
                "tool:triangle", "drag:900,400>1000,500",
                "tool:arrow", "drag:80,90>520,300",
                "select:tilt", "rotate-drag:30",
                "dbl:470,285", "type: soon", "endedit",
                "tool:pen", "drag:850,600>950,640", "tool:eraser", "drag:900,580>900,660",
                "icon:globe",
                "select:i4", "wait:200", "print"]),
            "perf": (20, "perf", "fit;wait:600;pan:6,0,240;zoom:1;wait:400;pan:-4,-2,240;fit;wait:400;select:a0;drag:200,30>900,330;wait:400;print"),
            # The app's paste check types into the terminal as it starts, which sets the agent working
            # until it says it is at its prompt again: Ask waits for that rather than mail it.
            "agent": (15, "settings", "select:cta;wait:1500;ask:make the button green;wait:3000;print;undo-agent;wait:1500;print"),
            "script": (16, "settings", [
                "select:note1", "nudge:10,0", "print",
                "drag:1000,100>1000,140",
                "dbl:40,80", "type: v2", "endedit",
                "tool:rect", "drag:1250,500>1350,580",
                "print",
                "undo", "redo",
                "select:note1,note2", "group", "print", "ungroup",
                "select:cta", "duplicate", "print", "delete",
                "select:row0", 'set:{"fill":"#FFF4D6"}',
                "undo", "undo", "redo", "redo",
                "wait:300", "print"]),
        }

        def scripted(shot, **extra):
            seconds, design, script = scripts[shot]
            return native(shot, design, script=script, seconds=seconds, **extra)

        def shot_media(shot):
            scripted(shot)
            got = call("design.get", {"design_id": designs["settings"]})
            images = [n for n in got["nodes"] if n["kind"] == "image"]
            assert len(images) == 1 and images[0]["w"] == 640 and images[0]["h"] == 400, images
            assert (Path(got["assets_dir"]) / images[0]["src"]).is_file()
            in_dark = [n for n in got["nodes"] if n.get("parent") == "dark" and n["kind"] == "frame"]
            assert len(in_dark) == 2, [n["id"] for n in in_dark]
            print("MEDIA_OK", images[0]["src"])

        def shot_shapes(shot):
            scripted(shot)
            got = engine_nodes("shapes")
            polys = [n for n in got.values() if n["kind"] == "polygon" and n["id"] not in ("tri", "star", "hex")]
            assert len(polys) == 1 and polys[0].get("sides") == 3, polys
            arrows = [n for n in got.values() if n["kind"] == "line" and n["id"] not in ("link", "curve")]
            assert len(arrows) == 1 and arrows[0].get("from") == "tri" and arrows[0].get("to") == "goal", arrows
            assert abs(got["tilt"]["rotation"] - 18) <= 2, got["tilt"]
            assert got["goal"]["text"] == "Ship it soon", got["goal"]
            assert not any(n["kind"] == "path" for n in got.values()), "the eraser left the stroke"
            assert any(n["kind"] == "icon" and n.get("icon") == "globe" for n in got.values())
            print("SHAPES_OK")

        def shot_perf(shot):
            run = scripted(shot, RELAY_DESIGN_TRACE=1)
            report = {kind: summary(series(run.log, kind)) for kind in ("layout", "content", "draw")}
            report = {k: v for k, v in report.items() if v}
            (OUT / "perf.json").write_text(json.dumps(report, indent=2))
            readings["perf"] = report
            print("PERF", json.dumps(report))

        def shot_agent(shot):
            run = scripted(shot)
            got = call("design.get", {"design_id": designs["settings"]})
            cta = next(n for n in got["nodes"] if n["id"] == "cta")
            scroll = call("session.scrollback", {"session": sname})["text"]
            assert "DESIGN_ASK " + sname + " idle" in run.log, run.log[-1500:]
            assert "[Relay design]" in scroll and "Selected: cta" in scroll and "Picture of it:" in scroll, scroll[-1500:]
            assert "agent applied" in scroll, scroll[-1500:]
            # The board's Undo put the agent's change back, through the audit.
            assert cta.get("fill") == "#151413", (cta, scroll[-800:])
            assert got["design"]["updated_by"] == "user", got["design"]
            assert run.scenes[-2]["cta"].get("fill") == "#2EC469", "the board did not read the agent's change"
            assert run.scenes[-1]["cta"].get("fill") == "#151413", "the board did not read the undo"
            print("AGENT_OK", sname)

        def shot_script(shot):
            run = scripted(shot)
            same_as_engine(run.scenes[-1], "settings")
            n = run.scenes[-1]
            assert n["note1"]["x"] in (900, 910), n["note1"]  # snaps to note2's left edge
            assert 20 <= n["note1"]["y"] - 40 <= 60, n["note1"]
            assert n["title"]["text"] == "Settings v2", n["title"]
            rects = [v for v in n.values() if v["kind"] == "rect" and v.get("parent") == "page-1"]
            assert len(rects) == 1 and 90 <= rects[0]["w"] <= 110 and 70 <= rects[0]["h"] <= 90, rects
            assert not any(v["kind"] == "group" for v in n.values()), "ungroup left a group"
            assert n["note1"]["parent"] == "page-1" and n["note2"]["parent"] == "page-1"
            assert any(v["kind"] == "group" for v in run.scenes[2].values()), "group made no group"
            assert n["row0"].get("fill") == "#FFF4D6", n["row0"]
            print("SCRIPT_OK", len(n), "nodes")

        shots = {"gallery": shot_simple, "gallery-narrow": shot_simple, "board": shot_simple, "board-select": shot_simple,
                 "board-narrow": shot_simple, "edit": shot_edit, "script": shot_script, "agent": shot_agent,
                 "agent-undo": shot_agent_undo, "agent-stale": shot_agent_stale, "agent-delete": shot_agent_delete,
                 "media": shot_media, "shapes": shot_shapes, "more": shot_more, "reuse": shot_reuse, "motion": shot_motion,
                 "render": shot_render, "glass": shot_glass, "pictures": shot_pictures, "perf": shot_perf}
        for shot in SHOTS:
            started = time.monotonic()
            try:
                shots[shot](shot)
                print(f"PASS {shot} ({time.monotonic() - started:.1f} s)", flush=True)
            except Exception:
                failed.append(shot)
                print(f"FAIL {shot} ({time.monotonic() - started:.1f} s)\n{traceback.format_exc()}", flush=True)
        (OUT / "design-readings.json").write_text(json.dumps(readings, indent=2))
    finally:
        for p in (kwin, engine):
            if p and p.poll() is None:
                p.terminate()
                try:
                    p.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    p.kill()
    print(f"{len(SHOTS) - len(failed)} of {len(SHOTS)} shots passed" + (f"; failed: {' '.join(failed)}" if failed else ""))
    sys.exit(1 if failed else 0)
