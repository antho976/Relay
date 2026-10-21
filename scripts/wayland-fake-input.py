#!/usr/bin/env python3
"""Real pointer and key input through KWin's org_kde_kwin_fake_input, on the private compositor
$WAYLAND_DISPLAY names (a `kwin_wayland --virtual` started with KWIN_WAYLAND_NO_PERMISSION_CHECKS=1,
as design-smoke.py starts it; never a desktop's). Coordinates are the output's. A raw Wayland
client, so it needs nothing installed.

    wayland-fake-input.py drag X Y DX DY [STEPS]   press at X,Y, move by DX,DY in STEPS, release
    wayland-fake-input.py click X Y
    wayland-fake-input.py move X Y
    wayland-fake-input.py keys CTRL KEY...         evdev key codes, Ctrl held when CTRL is 1"""
import os, socket, struct, sys, time
sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
sock.connect(os.path.join(os.environ["XDG_RUNTIME_DIR"], os.environ["WAYLAND_DISPLAY"]))
def s(text):
    b = text.encode() + b"\0"; return struct.pack("<I", len(b)) + b + b"\0" * (-len(b) % 4)
def send(obj, op, payload=b""):
    sock.sendall(struct.pack("<II", obj, (8 + len(payload)) << 16 | op) + payload)
fx = lambda v: struct.pack("<i", int(round(v * 256)))
u = lambda v: struct.pack("<I", v)
buf = b""
def events_until(callback_id):
    global buf
    out = []
    while True:
        while len(buf) >= 8:
            obj, word = struct.unpack("<II", buf[:8]); size = word >> 16
            if len(buf) < size: break
            out.append((obj, word & 0xffff, buf[8:size])); buf = buf[size:]
            if obj == callback_id: return out
        buf += sock.recv(65536)
next_id = [3]
def sync():
    cb = next_id[0]; next_id[0] += 1
    send(1, 0, u(cb)); return events_until(cb)
send(1, 1, u(2))                     # wl_display.get_registry -> 2
name = version = None
for obj, op, p in sync():
    if obj == 2 and op == 0:
        n, ln = struct.unpack("<II", p[:8]); iface = p[8:8 + ln - 1].decode()
        ver = struct.unpack("<I", p[8 + ln + (-ln % 4):][:4])[0]
        if iface == "org_kde_kwin_fake_input": name, version = n, ver
assert name, "no org_kde_kwin_fake_input"
FI = next_id[0]; next_id[0] += 1
send(2, 0, u(name) + s("org_kde_kwin_fake_input") + u(min(version, 5)) + u(FI))
send(FI, 0, s("relay smoke") + s("drives a private test compositor"))
sync()
def at(x, y): send(FI, 9, fx(x) + fx(y)); sync()
def button(down): send(FI, 2, u(0x110) + u(1 if down else 0)); sync()
def key(code, down): send(FI, 10, u(code) + u(1 if down else 0)); sync()
cmd, a = sys.argv[1], [float(v) for v in sys.argv[2:]]
if cmd == "drag":
    x, y, dx, dy = a[:4]; steps = int(a[4]) if len(a) > 4 else 25
    at(x, y); time.sleep(0.15); button(True); time.sleep(0.05)
    for i in range(1, steps + 1):
        at(x + dx * i / steps, y + dy * i / steps); time.sleep(0.016)
    time.sleep(0.1); button(False)
elif cmd == "move":
    at(a[0], a[1])
elif cmd == "click":
    at(a[0], a[1]); time.sleep(0.1); button(True); button(False)
elif cmd == "keys":
    ctrl = a[0] == 1
    if ctrl: key(29, True)
    for k in a[1:]:
        key(int(k), True); key(int(k), False); time.sleep(0.08)
    if ctrl: key(29, False)
time.sleep(0.2)
