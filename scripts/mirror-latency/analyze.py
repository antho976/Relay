#!/usr/bin/env python3
"""Stage-by-stage mirror latency from one run's `mirror_trace` lines.

    analyze.py engine.log native.log driver.log [--json]

Any of the logs may be missing stages: without the engine's trace the engine stages are blank and
the device's stage runs to the client's `recv`; without the driver's (probe.py) there is no clock
sync, so nothing is measured from the device's draw. Lines are matched on (mirror_id, seq); the
probe's barcode in each decoded picture (`probe`) is the device's CLOCK_MONOTONIC when it drew that
frame, turned into this host's clock by the driver's `sync` offset.
"""
import json
import re
import statistics
import sys

ANSI = re.compile(r"\x1b\[[0-9;]*m")
FIELD = re.compile(r'(\w+)=("[^"]*"|\S+)')


def events(paths):
    for path in paths:
        try:
            with open(path, errors="replace") as log:
                for line in log:
                    line = ANSI.sub("", line)
                    if "mirror_trace" not in line:
                        continue
                    fields = {k: v.strip('"') for k, v in FIELD.findall(line.split("mirror_trace", 1)[1])}
                    if "stage" in fields:
                        yield {k: (int(v) if re.fullmatch(r"-?\d+", v) else v) for k, v in fields.items()}
        except FileNotFoundError:
            pass


def summary(values):
    values = sorted(values)
    if not values:
        return None
    at = lambda q: values[min(len(values) - 1, int(q * len(values)))]
    return {"median": statistics.median(values), "p95": at(0.95), "max": values[-1], "n": len(values)}


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    as_json = "--json" in sys.argv
    stages = {}
    by_seq = {}
    phases, taps, syncs = [], [], []
    for e in events(args):
        stage = e["stage"]
        if stage == "sync":
            syncs.append(e["offset_ns"])
        elif stage == "phase":
            phases.append((e["mono_ns"], e["name"]))
        elif stage == "tap":
            taps.append(e)
        elif "seq" in e:
            # The first of each stage for a packet (a picture is `shown` once; `present` once).
            by_seq.setdefault((e["mirror_id"], e["seq"]), {}).setdefault(stage, e)
            stages.setdefault(stage, []).append(e)
    offset = statistics.mean(syncs) if syncs else None
    phases.sort()

    def phase_at(t):
        name = None
        for start, phase in phases:
            if start <= t:
                name = phase
        return name

    def drawn(bar, near):
        """The host-clock time the device drew barcode `bar` (24 bits of 0.1 ms), near `near`."""
        device_near = (near + offset) / 1e5
        base = int(device_near) & ~0xFFFFFF
        value = min((base + bar + k * 0x1000000 for k in (-1, 0, 1)), key=lambda v: abs(v - device_near))
        return value * 1e5 - offset

    ms = lambda a, b: (b - a) / 1e6
    rows = []
    for (mirror, seq), s in by_seq.items():
        dec = s.get("dec")
        if not dec or dec.get("probe", -1) < 0:
            continue
        row = {"mirror": mirror, "seq": seq, "phase": phase_at(dec["mono_ns"]), "dec": dec}
        row.update(s)
        if offset is not None:
            row["draw"] = drawn(dec["probe"], dec["mono_ns"])
        rows.append(row)
    animated = [r for r in rows if r["phase"] in (None, "anim")]

    report = {"pictures": len(rows), "animated": len(animated), "stages": {}, "touch": {}}

    def stage(name, pick):
        values = []
        for r in animated:
            try:
                value = pick(r)
            except (KeyError, TypeError):
                continue
            if value is not None:
                values.append(value)
        report["stages"][name] = summary(values)

    stage("device: draw -> engine has the header", lambda r: ms(r["draw"], r["read"]["header_ns"]))
    stage("engine: header -> payload read", lambda r: ms(r["read"]["header_ns"], r["read"]["mono_ns"]))
    stage("engine: read -> serialized (queue + base64 + JSON)", lambda r: ms(r["read"]["mono_ns"], r["serialize"]["mono_ns"]))
    stage("engine: serialized -> written", lambda r: ms(r["serialize"]["mono_ns"], r["flush"]["mono_ns"]))
    stage("socket: written -> client line complete", lambda r: ms(r["flush"]["mono_ns"], r["recv"]["line_ns"]))
    stage("device: draw -> client line complete", lambda r: ms(r["draw"], r["recv"]["line_ns"]))
    stage("client: JSON parse", lambda r: ms(r["recv"]["line_ns"], r["recv"]["mono_ns"]))
    stage("client: parsed -> admitted (base64 + gate)", lambda r: ms(r["recv"]["mono_ns"], r["in"]["mono_ns"]))
    stage("client: admitted -> decode starts", lambda r: ms(r["in"]["mono_ns"], r["dec"]["in_ns"]))
    stage("decode: unit in -> picture out", lambda r: ms(r["dec"]["in_ns"], r["dec"]["mono_ns"]))
    stage("gtk: picture out -> main loop has it", lambda r: ms(r["dec"]["mono_ns"], r["paint"]["got_ns"]))
    stage("gtk: texture built and set", lambda r: ms(r["paint"]["got_ns"], r["paint"]["mono_ns"]))
    stage("gtk: set -> frame painted", lambda r: ms(r["paint"]["mono_ns"], r["shown"]["mono_ns"]))
    stage("gtk: paint (snapshot + upload + render)", lambda r: ms(r["shown"]["paint_ns"], r["shown"]["mono_ns"]) if r["shown"].get("paint_ns") else None)
    stage("compositor: painted -> presented", lambda r: ms(r["shown"]["mono_ns"], r["present"]["mono_ns"]) if r["present"]["mono_ns"] else None)
    stage("HOST: line complete -> presented", lambda r: ms(r["recv"]["line_ns"], r["present"]["mono_ns"]) if r["present"]["mono_ns"] else None)
    stage("TOTAL: draw -> decoded", lambda r: ms(r["draw"], r["dec"]["mono_ns"]))
    stage("TOTAL: draw -> painted", lambda r: ms(r["draw"], r["shown"]["mono_ns"]))
    stage("TOTAL: draw -> presented", lambda r: ms(r["draw"], r["present"]["mono_ns"]) if r["present"]["mono_ns"] else None)
    shown = sum(1 for r in animated if "shown" in r)
    report["shown_of_decoded"] = f"{shown}/{len(animated)}"
    sizes = [e["len"] for e in stages.get("in", []) if not e.get("flags", 0) & 1]
    report["picture_bytes"] = summary(sizes)

    # Touch to photon: a tap flips the probe's field; the first picture with the other field.
    decoded = sorted(((r["dec"]["mono_ns"], r) for r in rows), key=lambda x: x[0])
    for phase in ("animtaps", "stilltaps"):
        measures = {k: [] for k in ("tap -> device draw", "tap -> decoded", "tap -> painted", "tap -> presented")}
        for tap in (t for t in taps if phase_at(t["sent_ns"]) == phase):
            before = [r for t, r in decoded if t < tap["sent_ns"]]
            if not before:
                continue
            state = before[-1]["dec"]["field"]
            flip = next((r for t, r in decoded if t > tap["sent_ns"] and r["dec"]["field"] != state), None)
            if flip is None:
                continue
            sent = tap["sent_ns"]
            if "draw" in flip:
                measures["tap -> device draw"].append(ms(sent, flip["draw"]))
            measures["tap -> decoded"].append(ms(sent, flip["dec"]["mono_ns"]))
            if "shown" in flip:
                measures["tap -> painted"].append(ms(sent, flip["shown"]["mono_ns"]))
            if "present" in flip and flip["present"]["mono_ns"]:
                measures["tap -> presented"].append(ms(sent, flip["present"]["mono_ns"]))
        report["touch"][phase] = {k: summary(v) for k, v in measures.items()}

    if as_json:
        print(json.dumps(report, indent=1))
        return
    line = lambda s: "n/a" if not s else f"med {s['median']:7.2f}  p95 {s['p95']:7.2f}  max {s['max']:7.2f}  n={s['n']}"
    print(f"pictures with the probe: {report['pictures']} ({report['animated']} animating), shown {report['shown_of_decoded']}")
    if report["picture_bytes"]:
        print(f"picture packet bytes: median {report['picture_bytes']['median']:.0f}, max {report['picture_bytes']['max']}")
    print("\n== animation, ms ==")
    for name, s in report["stages"].items():
        print(f"  {name:52s} {line(s)}")
    for phase, measures in report["touch"].items():
        print(f"\n== {phase}, ms ==")
        for name, s in measures.items():
            print(f"  {name:52s} {line(s)}")


if __name__ == "__main__":
    main()
