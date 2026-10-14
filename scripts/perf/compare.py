#!/usr/bin/env python3
"""Compare two baseline runs: what got slower, what got faster, what changed shape.

    python3 scripts/perf/compare.py docs/perf/runs/2026-09-19 docs/perf/runs/2026-10-03 [--threshold 0.2]

Instructions are the regression signal to trust: they do not move with machine load. They come
from the CPU's counters (`counters.jsonl`) when both runs have them, else from callgrind. Wall
time is reported beside them for the paths that wait on the kernel or a child.
"""
import argparse
import json
from pathlib import Path


def read_jsonl(path):
    if not path.exists():
        return {}
    return {json.loads(l)["name"]: json.loads(l) for l in path.read_text().splitlines() if l.strip()}


def read_json(path):
    return json.loads(path.read_text()) if path.exists() else {}


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("before", type=Path)
    ap.add_argument("after", type=Path)
    ap.add_argument("--threshold", type=float, default=0.2, help="relative change worth printing")
    args = ap.parse_args()
    a_native, b_native = read_jsonl(args.before / "native.jsonl"), read_jsonl(args.after / "native.jsonl")
    a_prof, b_prof = read_json(args.before / "profiles.json"), read_json(args.after / "profiles.json")
    a_cnt, b_cnt = read_jsonl(args.before / "counters.jsonl"), read_jsonl(args.after / "counters.jsonl")

    def counted(rows, name):
        c = (rows.get(name) or {}).get("counters") or {}
        return c["instructions"]["p50"] if c.get("instructions") else None

    # Wall time only compares on one machine and toolchain, and --quick runs a quarter of the
    # iterations. Say so up front rather than let the rows below look like regressions.
    a_env, b_env = read_json(args.before / "environment.json"), read_json(args.after / "environment.json")
    a_env.setdefault("quick", False)  # runs from before it was recorded were full runs
    b_env.setdefault("quick", False)
    for key in ("cpu", "cores", "mem_gib", "kernel", "rustc", "profile", "container", "quick"):
        if a_env.get(key) != b_env.get(key):
            print(f"warning: {key} differs: {a_env.get(key)!r} → {b_env.get(key)!r}")

    rows = []
    for name in sorted(set(a_native) | set(b_native)):
        a, b = a_native.get(name), b_native.get(name)
        if not a or not b:
            rows.append((name, "only in " + ("before" if a else "after"), None, None, None))
            continue
        wall = b["wall_ns"]["p50"] / max(a["wall_ns"]["p50"], 1)
        allocs = b["allocs_per_iter"] / max(a["allocs_per_iter"], 1)
        ir = None
        if counted(a_cnt, name) and counted(b_cnt, name):
            ir = counted(b_cnt, name) / counted(a_cnt, name)
        elif name in a_prof and name in b_prof and a_prof[name].get("ir_exact", True) and b_prof[name].get("ir_exact", True):
            ir = b_prof[name]["ir_per_iter"] / max(a_prof[name]["ir_per_iter"], 1)
        # Counts scale with the iteration count; what each iteration came back with does not.
        a_iters, b_iters = max(a.get("iters", 1), 1), max(b.get("iters", 1), 1)
        same = set(a.get("codes") or {}) == set(b.get("codes") or {}) and abs(a["ok"] / a_iters - b["ok"] / b_iters) < 0.01
        outcome = "" if same else f"outcome changed: ok {a['ok']}/{a_iters} {a.get('codes')} → ok {b['ok']}/{b_iters} {b.get('codes')}"
        rows.append((name, outcome, wall, allocs, ir))

    def flag(x):
        if x is None:
            return "–"
        mark = "  ▲" if x > 1 + args.threshold else ("  ▼" if x < 1 - args.threshold else "")
        return f"{x:.2f}×{mark}"

    print(f"{'scenario':42} {'wall p50':>12} {'allocs':>12} {'instructions':>14}  note")
    for name, note, wall, allocs, ir in rows:
        interesting = note or any(x is not None and abs(x - 1) > args.threshold for x in (wall, allocs, ir))
        if interesting:
            print(f"{name:42} {flag(wall):>12} {flag(allocs):>12} {flag(ir):>14}  {note}")
    print()
    print("▲ slower / more, ▼ faster / fewer, beyond the threshold. Rows inside the threshold are not shown.")


if __name__ == "__main__":
    main()
