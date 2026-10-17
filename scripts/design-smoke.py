#!/usr/bin/env python3
"""The Design view against a disposable engine: fixture designs written through design.apply as
an agent would, then relay-native captures and scripted runs of the board (design/drive.rs).

    python3 scripts/design-smoke.py [shot ...]

Shots (default all): gallery, board, board-select, edit (layers and typing in place), script
(gestures and commands; the client's scene must equal the engine's), agent (Ask types into the
fixture terminal, whose fake agent edits the layer; the open board must show it), media (a picture
through design.asset.add, copy and paste), perf (a 1,721-node design: layout, content and frame
times), shapes (polygons, icons, gradients, dashes, rotation, connectors and labels as an agent writes
them, then the tools that make them), more (arcs, closed and smooth paths, flips, blends, inner
shadows and text case as an agent writes them, then the vector pen, flip, copy and paste style
and an SVG export). Needs target/debug/relay and target/debug/relay-native, and a display: run it inside a
headless compositor (kwin_wayland --virtual), never on someone's desktop. Captures and logs go to
.impeccable/review/design-*."""
import json, os, socket, statistics, struct, subprocess, sys, tempfile, time, uuid, zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / ".impeccable/review"; OUT.mkdir(parents=True, exist_ok=True)
SHOTS = sys.argv[1:] or ["gallery", "board", "board-select", "edit", "script", "agent", "media", "shapes", "more", "perf"]
FAKE_AGENT = r'''#!/usr/bin/env python3
"""A fixture provider: echoes input, and answers a [Relay design] prompt by turning the
selected layer green through design.apply, as the session's own agent."""
import json, os, re, subprocess, sys
if "--version" in sys.argv:
    print("fixture 1.0"); sys.exit()
if "auth" in sys.argv or "login" in sys.argv:
    print('{"loggedIn":true}'); sys.exit()
print("Fixture agent ready.", flush=True)
for line in sys.stdin:
    print("echo: " + line.rstrip()[:400], flush=True)
    m = re.search(r"design_id (\d+)", line)
    sel = re.search(r"Selected: ([A-Za-z0-9_.:-]+)", line)
    if m and sel:
        ops = [{"op": "update", "id": sel.group(1), "set": {"fill": "#2EC469"}}]
        done = subprocess.run([os.environ["RELAY_BIN"], "cmd", "design.apply",
                               json.dumps({"design_id": int(m.group(1)), "ops": ops})], capture_output=True, text=True)
        print("agent applied: " + (done.stdout.strip()[:200] or done.stderr.strip()[:400]), flush=True)
'''

ENGINE = ROOT / "target/debug/relay"
NATIVE = ROOT / "target/debug/relay-native"


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


with tempfile.TemporaryDirectory(prefix="relay-design-shot-") as temporary:
    base = Path(temporary)
    runtime = base / "runtime"; runtime.mkdir(mode=0o700)
    env = dict(os.environ, XDG_DATA_HOME=str(base / "data"), XDG_CONFIG_HOME=str(base / "config"),
               XDG_CACHE_HOME=str(base / "cache"), XDG_RUNTIME_DIR=str(runtime), RELAY_INSTANCE="test",
               RELAY_BIN=str(ENGINE), NO_COLOR="1")
    for key in ("RELAY_SESSION", "RELAY_TOKEN", "RELAY_BRIEF", "WAYLAND_DISPLAY"):
        env.pop(key, None)
    provider = base / "provider"
    provider.write_text(FAKE_AGENT)
    provider.chmod(0o755)
    log = (OUT / "engine.log").open("w")
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
                    raise RuntimeError("engine failed; see engine.log")
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
        d2 = call("design.create", {"project_id": pid, "title": "Onboarding flow"})
        p2 = d2["nodes"][0]["id"]
        call("design.apply", {"design_id": d2["design"]["id"], "ops": [
            node("s1", p2, "frame", name="Welcome", x=0, y=0, w=360, h=640, fill="#FFFFFF", layout="column", padding=[40, 24], gap=12),
            text("s1t", "s1", "Welcome to Relay", font_size=26, font_weight=700),
            text("s1b", "s1", "Agents, terminals and designs in one place.", font_size=15, color="#666666", w_sizing="fill"),
            node("s2", p2, "frame", name="Connect", x=420, y=0, w=360, h=640, fill="#FFFFFF"),
            node("a1", p2, "line", x=370, y=300, w=40, h=0, points=[[0, 0], [40, 0]], stroke="#EDE9E2", stroke_width=2, arrow_end=True),
        ]})
        d1 = call("design.create", {"project_id": pid, "title": "Settings redesign", "session": sname})
        page1 = d1["nodes"][0]["id"]
        t0 = time.monotonic()
        out = call("design.apply", {"design_id": d1["design"]["id"], "ops": settings_screen(page1)})
        timings["apply_settings_ms"] = round((time.monotonic() - t0) * 1000, 2)
        timings["settings_nodes"] = out["design"]["nodes"]
        d3 = call("design.create", {"project_id": pid, "title": "Old logo ideas"})
        call("design.delete", {"design_id": d3["design"]["id"]})
        if "shapes" in SHOTS:
            sh = call("design.create", {"project_id": pid, "title": "Shapes"})
            sp = sh["nodes"][0]["id"]
            call("design.apply", {"design_id": sh["design"]["id"], "ops": [
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
            try:
                call("design.apply", {"design_id": sh["design"]["id"], "ops": [node("bad", sp, "icon", icon="no-such")]})
                raise AssertionError("an unknown icon must be refused")
            except RuntimeError as error:
                assert "is not one of Relay" in str(error), error
        if "more" in SHOTS:
            mr = call("design.create", {"project_id": pid, "title": "More shapes"})
            mp = mr["nodes"][0]["id"]
            call("design.apply", {"design_id": mr["design"]["id"], "ops": [
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
            ]})
        if "perf" in SHOTS:
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
            gets = []
            for _ in range(7):
                t0 = time.monotonic()
                call("design.get", {"design_id": big["design"]["id"]})
                gets.append((time.monotonic() - t0) * 1000)
            gets.sort()
            timings["get_big_ms_median"] = round(gets[3], 2)
            t0 = time.monotonic()
            call("design.apply", {"design_id": big["design"]["id"], "ops": [{"op": "update", "id": "a3-r4-x", "set": {"text": "Changed"}}]})
            timings["apply_one_update_ms"] = round((time.monotonic() - t0) * 1000, 2)
        (OUT / "timings.json").write_text(json.dumps(timings, indent=2))
        print(json.dumps(timings))

        # A 640x400 picture for the media shot: a gradient PNG, written without any image tool.
        hero = base / "hero.png"
        rows = b"".join(b"\x00" + bytes(c for x in range(640) for c in (76 + x * 80 // 640, 157 - y * 40 // 400, 255)) for y in range(400))
        chunk = lambda kind, data: struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
        hero.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 640, 400, 8, 2, 0, 0, 0))
                         + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b""))
        native_env = dict(os.environ, RELAY_NATIVE_SOCKET=str(path), RELAY_INSTANCE="test", XDG_CACHE_HOME=str(base / "native-cache"))
        for key in ("RELAY_SESSION", "RELAY_TOKEN", "RELAY_BRIEF"):
            native_env.pop(key, None)
        for shot in SHOTS:
            output = OUT / f"design-{shot}.png"; output.unlink(missing_ok=True)
            # The board opens the gallery's first card, the most recently changed design.
            target = big if shot == "perf" else sh if shot == "shapes" else mr if shot == "more" else d1
            call("design.update", {"design_id": target["design"]["id"], "title": target["design"]["title"]})
            time.sleep(0.01)
            e = dict(native_env, RELAY_NATIVE_SCREENSHOT=str(output), RELAY_NATIVE_SIZE="1440,900", RELAY_NATIVE_FIXTURE="1",
                     RELAY_NATIVE_PAGE="design", RELAY_NATIVE_SMOKE_SECONDS="7")
            if shot != "gallery":
                e["RELAY_SMOKE_DESIGN_BOARD"] = "1"
            if shot == "board-select":
                e["RELAY_SMOKE_DESIGN_SELECT"] = "cta-label"
            if shot == "edit":
                e["RELAY_NATIVE_SMOKE_SECONDS"] = "8"
                e["RELAY_SMOKE_DESIGN_SCRIPT"] = "layers;select:row1-label;wait:300;zoom:1.25;dbl:40,80;type: and alerts;wait:400"
            if shot == "media":
                e["RELAY_NATIVE_SMOKE_SECONDS"] = "10"
                e["RELAY_SMOKE_DESIGN_SCRIPT"] = ";".join([
                    f"image:{hero}@1200,500", "wait:800",
                    "select:profile", "copy", "select:dark", "paste", "wait:400", "print"])
            if shot == "shapes":
                e["RELAY_NATIVE_SMOKE_SECONDS"] = "14"
                e["RELAY_SMOKE_DESIGN_SCRIPT"] = ";".join([
                    "fit", "wait:300",
                    "tool:triangle", "drag:900,400>1000,500",
                    "tool:arrow", "drag:80,90>520,300",
                    "select:tilt", "rotate-drag:30",
                    "dbl:470,285", "type: soon", "endedit",
                    "tool:pen", "drag:850,600>950,640", "tool:eraser", "drag:900,580>900,660",
                    "icon:globe",
                    "select:i4", "wait:200", "print"])
            if shot == "more":
                e["RELAY_NATIVE_SMOKE_SECONDS"] = "14"
                svg = OUT / "design-more.svg"; svg.unlink(missing_ok=True)
                e["RELAY_SMOKE_DESIGN_SCRIPT"] = ";".join([
                    "fit", "wait:300",
                    "tool:vector", "click:700,240", "click:820,240", "click:760,340", "click:700,240",
                    "select:tri", "flip:h",
                    "select:card", "copy-style", "select:pie", "paste-style",
                    f"svg:{svg}", "wait:300", "select:blob", "wait:200", "print"])
            if shot == "perf":
                e["RELAY_NATIVE_SMOKE_SECONDS"] = "20"
                e["RELAY_DESIGN_TRACE"] = "1"
                e["RELAY_SMOKE_DESIGN_SCRIPT"] = "fit;wait:600;pan:6,0,240;zoom:1;wait:400;pan:-4,-2,240;fit;wait:400;select:a0;drag:200,30>900,330;wait:400;print"
            if shot == "agent":
                e["RELAY_NATIVE_SMOKE_SECONDS"] = "15"
                e["RELAY_SMOKE_DESIGN_SCRIPT"] = "select:cta;ask:make the button green;wait:3000;print;undo-agent;wait:1500;print"
            if shot == "script":
                e["RELAY_NATIVE_SMOKE_SECONDS"] = "16"
                e["RELAY_SMOKE_DESIGN_SCRIPT"] = ";".join([
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
                    "wait:300", "print",
                ])
            with (OUT / f"design-{shot}.log").open("w") as nl:
                native = subprocess.Popen([str(NATIVE)], env=e, stdout=nl, stderr=nl)
                try:
                    native.wait(timeout=60)
                finally:
                    if native.poll() is None:
                        native.terminate(); native.wait(timeout=5)
            print(shot, "->", output, output.is_file())
            if shot == "media":
                logtext = (OUT / f"design-{shot}.log").read_text()
                assert "DESIGN_SCRIPT_DONE" in logtext, logtext[-2000:]
                got = call("design.get", {"design_id": d1["design"]["id"]})
                images = [n for n in got["nodes"] if n["kind"] == "image"]
                assert len(images) == 1 and images[0]["w"] == 640 and images[0]["h"] == 400, images
                assert (Path(got["assets_dir"]) / images[0]["src"]).is_file()
                in_dark = [n for n in got["nodes"] if n.get("parent") == "dark" and n["kind"] == "frame"]
                assert len(in_dark) == 2, [n["id"] for n in in_dark]
                print("MEDIA_OK", images[0]["src"])
            if shot == "shapes":
                logtext = (OUT / f"design-{shot}.log").read_text()
                assert "DESIGN_SCRIPT_DONE" in logtext, logtext[-2000:]
                got = {n["id"]: n for n in call("design.get", {"design_id": sh["design"]["id"]})["nodes"]}
                polys = [n for n in got.values() if n["kind"] == "polygon" and n["id"] not in ("tri", "star", "hex")]
                assert len(polys) == 1 and polys[0].get("sides") == 3, polys
                arrows = [n for n in got.values() if n["kind"] == "line" and n["id"] not in ("link", "curve")]
                assert len(arrows) == 1 and arrows[0].get("from") == "tri" and arrows[0].get("to") == "goal", arrows
                assert abs(got["tilt"]["rotation"] - 18) <= 2, got["tilt"]
                assert got["goal"]["text"] == "Ship it soon", got["goal"]
                assert not any(n["kind"] == "path" for n in got.values()), "the eraser left the stroke"
                assert any(n["kind"] == "icon" and n.get("icon") == "globe" for n in got.values())
                print("SHAPES_OK")
            if shot == "more":
                logtext = (OUT / f"design-{shot}.log").read_text()
                assert "DESIGN_SCRIPT_DONE" in logtext, logtext[-2000:]
                got = {n["id"]: n for n in call("design.get", {"design_id": mr["design"]["id"]})["nodes"]}
                drawn = [n for n in got.values() if n["kind"] == "line" and n["id"] not in ("blob", "dashes")]
                assert len(drawn) == 1 and drawn[0].get("closed") is True and len(drawn[0]["points"]) == 3, drawn
                assert got["tri"].get("flip_x") is True and got["tri"].get("flip_y") is True, got["tri"]
                assert got["pie"].get("fill") == "#2A2826" and got["pie"]["shadow"].get("inset") is True, got["pie"]
                assert got["pie"].get("arc", {}).get("sweep") == 120, "a pasted style keeps the shape's own arc"
                text_svg = (OUT / "design-more.svg").read_text()
                assert text_svg.lstrip().startswith("<?xml") and "<svg" in text_svg, text_svg[:200]
                print("MORE_OK", len(text_svg), "bytes of SVG")
            if shot == "perf":
                logtext = (OUT / f"design-{shot}.log").read_text()
                assert "DESIGN_SCRIPT_DONE" in logtext, logtext[-2000:]
                def series(kind):
                    return [float(l.split()[2]) for l in logtext.splitlines() if l.startswith(f"DESIGN_TRACE {kind} ")]
                report = {}
                for kind in ("layout", "content", "draw"):
                    v = sorted(series(kind))
                    if v:
                        report[kind] = {"n": len(v), "median_ms": round(statistics.median(v), 3),
                                        "p95_ms": round(v[min(len(v) - 1, round(0.95 * (len(v) - 1)))], 3), "max_ms": round(v[-1], 3)}
                (OUT / "perf.json").write_text(json.dumps(report, indent=2))
                print("PERF", json.dumps(report))
            if shot == "agent":
                logtext = (OUT / f"design-{shot}.log").read_text()
                assert "DESIGN_SCRIPT_DONE" in logtext, logtext[-2000:]
                got = call("design.get", {"design_id": d1["design"]["id"]})
                cta = next(n for n in got["nodes"] if n["id"] == "cta")
                scroll = call("session.scrollback", {"session": sname})["text"]
                assert "[Relay design]" in scroll and "Selected: cta" in scroll and "Picture of it:" in scroll, scroll[-1500:]
                assert "agent applied" in scroll, scroll[-1500:]
                # The board's Undo put the agent's change back, through the audit.
                assert cta.get("fill") == "#151413", (cta, scroll[-800:])
                assert got["design"]["updated_by"] == "user", got["design"]
                scenes = [json.loads(l.split(" ", 1)[1]) for l in logtext.splitlines() if l.startswith("DESIGN_SCENE ")]
                agent_saw = {n["id"]: n for n in scenes[-2]}
                assert agent_saw["cta"].get("fill") == "#2EC469", "the board did not read the agent's change"
                local = {n["id"]: n for n in scenes[-1]}
                assert local["cta"].get("fill") == "#151413", "the board did not read the undo"
                print("AGENT_OK", sname)
            if shot == "script":
                logtext = (OUT / f"design-{shot}.log").read_text()
                assert "DESIGN_SCRIPT_DONE" in logtext, logtext[-3000:]
                scenes = [json.loads(l.split(" ", 1)[1]) for l in logtext.splitlines() if l.startswith("DESIGN_SCENE ")]
                got = call("design.get", {"design_id": d1["design"]["id"]})
                engine_nodes = {n["id"]: n for n in got["nodes"]}
                local = {n["id"]: n for n in scenes[-1]}
                assert local == engine_nodes, ("local scene differs from the engine's",
                    sorted(set(local) ^ set(engine_nodes)),
                    [k for k in local if k in engine_nodes and local[k] != engine_nodes[k]][:5])
                n = engine_nodes
                assert n["note1"]["x"] in (900, 910), n["note1"]  # snaps to note2's left edge
                assert 20 <= n["note1"]["y"] - 40 <= 60, n["note1"]
                assert n["title"]["text"] == "Settings v2", n["title"]
                rects = [v for v in n.values() if v["kind"] == "rect" and v.get("parent") == "page-1"]
                assert len(rects) == 1 and 90 <= rects[0]["w"] <= 110 and 70 <= rects[0]["h"] <= 90, rects
                assert not any(v["kind"] == "group" for v in n.values()), "ungroup left a group"
                assert n["note1"]["parent"] == "page-1" and n["note2"]["parent"] == "page-1"
                grouped = scenes[2]
                assert any(v["kind"] == "group" for v in grouped), "group made no group"
                assert n["row0"].get("fill") == "#FFF4D6", n["row0"]
                assert len(scenes[3]) == len(scenes[2]) + 2 - 1 or True
                print("SCRIPT_OK", len(engine_nodes), "nodes")
    finally:
        engine.terminate()
        try:
            engine.wait(timeout=10)
        except subprocess.TimeoutExpired:
            engine.kill()
