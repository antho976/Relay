#!/usr/bin/env python3
"""Drive the latency probe on a device while Relay mirrors it, and log what was done when.

The mirror must already be open on the device, with the engine and the native client tracing
(README.md). This script starts the probe app, syncs the device's clock with this host's
(both CLOCK_MONOTONIC), lets the probe animate, then taps its touch field through the engine —
first while it animates, then on a still screen — and writes its own `mirror_trace` lines
(`sync`, `phase`, `tap`) for analyze.py.

    probe.py --device SERIAL --native-log native.log --out driver.log

The taps go to the mirror the native log names, at the picture size it last decoded, over the
engine's socket (`--socket`, default the `dev` instance's) as `device.mirror.input` — the op the
window itself sends.
"""
import argparse
import json
import os
import re
import socket
import subprocess
import sys
import time
import uuid

ANSI = re.compile(r"\x1b\[[0-9;]*m")
FIELD = re.compile(r'(\w+)=("[^"]*"|\S+)')


def adb(serial, *args):
    return subprocess.run(["adb", "-s", serial, *args], check=True, capture_output=True, text=True).stdout


def probe_socket(port):
    s = socket.create_connection(("127.0.0.1", port))
    s.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
    return s


def command(port, byte):
    """One probe command ('a'nim, 's'till, 'h'eavy, 'l'ight, 'c'hart, 'n'o chart)."""
    with probe_socket(port) as s:
        s.sendall(byte)
        s.recv(8)


def clock_sync(port, rounds=400):
    """Device minus host CLOCK_MONOTONIC, from the round trip with the least delay."""
    best = (0, float("inf"))
    with probe_socket(port) as s:
        for _ in range(rounds):
            t0 = time.monotonic_ns()
            s.sendall(b"x")
            reply = b""
            while len(reply) < 8:
                reply += s.recv(8 - len(reply))
            t1 = time.monotonic_ns()
            if t1 - t0 < best[1]:
                best = (int.from_bytes(reply, "big", signed=True) - (t0 + t1) // 2, t1 - t0)
    return best


def last_picture(native_log):
    """(mirror_id, width, height) of the newest `dec` line in the native client's log."""
    found = None
    with open(native_log, errors="replace") as log:
        for line in log:
            line = ANSI.sub("", line)
            if "mirror_trace" in line and 'stage="dec"' in line:
                fields = dict(FIELD.findall(line))
                found = (int(fields["mirror_id"]), int(fields["width"]), int(fields["height"]))
    return found


def default_socket(instance):
    runtime = os.environ.get("XDG_RUNTIME_DIR") or f"/run/user/{os.getuid()}"
    return os.path.join(runtime, "relay-v4", f"{instance}.sock")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--device", required=True, help="adb serial of the mirrored device")
    parser.add_argument("--native-log", required=True, help="the native client's stdout (RUST_LOG=mirror_trace=info)")
    parser.add_argument("--out", required=True, help="where this script's own trace lines go")
    parser.add_argument("--socket", help="the engine's socket (default: the --instance's)")
    parser.add_argument("--instance", default="dev")
    parser.add_argument("--anim", type=float, default=10, help="seconds of animation before tapping")
    parser.add_argument("--taps", type=int, default=25, help="taps in each phase")
    parser.add_argument("--gap", type=float, default=0.7, help="seconds between taps")
    parser.add_argument("--heavy", action="store_true", help="1500 random squares a frame: an encoder under load")
    parser.add_argument("--chart", type=float, default=0, help="end on a still color chart for this many seconds")
    args = parser.parse_args()

    out = open(args.out, "a", buffering=1)

    def log(stage, **fields):
        text = " ".join(f"{k}={v}" for k, v in fields.items())
        out.write(f"mirror_trace: stage=\"{stage}\" mono_ns={time.monotonic_ns()} {text}\n")

    port = int(adb(args.device, "forward", "tcp:0", "localabstract:latprobe").strip())
    try:
        adb(args.device, "shell", "am", "start", "-n", "dev.relay.latprobe/.Probe")
        time.sleep(1.5)
        offset, rtt = clock_sync(port)
        log("sync", offset_ns=offset, rtt_ns=rtt)
        command(port, b"n")
        command(port, b"h" if args.heavy else b"l")
        command(port, b"a")
        log("phase", name="anim")
        time.sleep(args.anim)

        bus = socket.socket(socket.AF_UNIX)
        bus.connect(args.socket or default_socket(args.instance))
        replies = bus.makefile("rb")

        def tap(action, point, size):
            event = {"type": "touch", "action": action, "x": point[0], "y": point[1], "w": size[0], "h": size[1],
                     "pressure": 0.0 if action == 1 else 1.0}
            request = {"v": 1, "id": str(uuid.uuid4()), "actor": "user", "op": "device.mirror.input",
                       "payload": {"mirror_id": mirror, "event": event}}
            sent = time.monotonic_ns()
            bus.sendall(json.dumps(request).encode() + b"\n")
            return sent, json.loads(replies.readline())

        for phase in ("animtaps", "stilltaps"):
            if phase == "stilltaps":
                command(port, b"s")
                time.sleep(1)
            log("phase", name=phase)
            for _ in range(args.taps):
                picture = last_picture(args.native_log)
                if picture is None:
                    sys.exit("No decoded picture in the native log: is RELAY_MIRROR_TRACE=1 RUST_LOG=mirror_trace=info set?")
                mirror, width, height = picture
                point = (width * 2 // 3, height // 2)
                sent, reply = tap(0, point, (width, height))
                if not reply.get("ok", True):
                    sys.exit(f"device.mirror.input was refused: {reply}")
                log("tap", sent_ns=sent, mirror_id=mirror)
                time.sleep(0.06)
                tap(1, point, (width, height))
                time.sleep(args.gap)
        command(port, b"a")
        log("phase", name="end")
        offset, rtt = clock_sync(port)
        log("sync", offset_ns=offset, rtt_ns=rtt)
        if args.chart:
            command(port, b"s")
            command(port, b"c")
            log("phase", name="chart")
            time.sleep(args.chart)
            command(port, b"n")
            command(port, b"a")
    finally:
        adb(args.device, "forward", "--remove", f"tcp:{port}")


if __name__ == "__main__":
    main()
