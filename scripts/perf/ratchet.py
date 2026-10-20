#!/usr/bin/env python3
"""A ratchet on the design rows' instruction counts (docs/perf/DESIGN-2026-10-20-scale.md).

    python3 scripts/perf/ratchet.py                      # build, run the guarded rows 3 times, check
    python3 scripts/perf/ratchet.py --runs a.jsonl b.jsonl c.jsonl   # check runs already taken
    python3 scripts/perf/ratchet.py --lower              # and lower each ceiling a row came under

A row's figure is the median, over the runs, of its per-iteration median of user-space
instructions (`perf run --counters`, dev profile): the measured call alone, on every thread it
touches. The check fails when a figure is over its ceiling in `ratchet.json` by more than the
tolerance, and says which. `--lower` writes the figure of every row that came in under its
ceiling by more than the tolerance, so the next change is held to the win. Raising a ceiling is
an edit of `ratchet.json` by hand, with the reason in the commit: the work got bigger on purpose.
A new toolchain or dependency moves every count; refresh them all in one commit (`--reset`).

Three runs of the same build agree within 0.7% on every guarded row (the hash maps' random seeds
are the spread), so the tolerance is 1.5%. Needs the CPU's counters: `perf_event_paranoid` at 2
or less, and a PMU, which CI's virtual machines lack, so this is a check to run before merging a
change to the design engine or the client's scene, not a CI job.
"""

import argparse
import json
import os
import statistics
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CEILINGS = Path(__file__).with_name("ratchet.json")
ROWS = ["op.design*", "path.design*"]


def take_runs(n):
    subprocess.run(["cargo", "build", "-q", "-p", "relay-core", "--example", "perf"], cwd=ROOT, check=True)
    out = ROOT / "target" / "perf-ratchet"
    out.mkdir(parents=True, exist_ok=True)
    runs = []
    for i in range(n):
        path = out / f"run-{i + 1}.jsonl"
        print(f"run {i + 1} of {n} -> {path}", file=sys.stderr)
        subprocess.run([str(ROOT / "target/debug/examples/perf"), "run", *ROWS, "--counters", "--out", str(path)],
                       cwd=ROOT, check=True, stderr=subprocess.DEVNULL)
        runs.append(path)
    return runs


def figures(paths):
    """Each row's median over the runs of its per-iteration median, and the runs' spread."""
    seen = {}
    for path in paths:
        for line in Path(path).read_text().splitlines():
            row = json.loads(line)
            counted = (row.get("counters") or {}).get("instructions")
            if not counted:
                sys.exit(f"{path}: {row['name']} has no instruction count; take the runs with --counters")
            if row["ok"] != row["iters"]:
                sys.exit(f"{path}: {row['name']} failed {row['iters'] - row['ok']} of {row['iters']} times: {row['codes']}")
            seen.setdefault(row["name"], []).append(counted["p50"])
    return {name: (statistics.median(v), (max(v) - min(v)) / statistics.median(v)) for name, v in seen.items()}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--runs", nargs="+", help="runs already taken (perf run ... --counters --out FILE)")
    parser.add_argument("-n", type=int, default=3, help="runs to take when none are given")
    parser.add_argument("--lower", action="store_true", help="lower each ceiling a row came under")
    parser.add_argument("--reset", action="store_true", help="set every ceiling to this build's figure")
    args = parser.parse_args()
    if open("/proc/sys/kernel/perf_event_paranoid").read().strip() not in ("-1", "0", "1", "2"):
        sys.exit("perf_event_paranoid is over 2: the counters cannot be read")
    got = figures(args.runs or take_runs(args.n))
    book = json.loads(CEILINGS.read_text())
    tolerance, ceilings = book["tolerance"], book["rows"]
    over, lowered = [], []
    for name in sorted(set(got) | set(ceilings)):
        if name not in got:
            print(f"  {name:44s} not run")
            continue
        figure, spread = got[name]
        ceiling = ceilings.get(name)
        if ceiling is None or args.reset:
            ceilings[name] = round(figure)
            print(f"  {name:44s} {figure / 1e6:11.2f} M  (new ceiling)")
            continue
        change = figure / ceiling - 1
        mark = ""
        if change > tolerance:
            over.append(name)
            mark = "  OVER"
        elif change < -tolerance and args.lower:
            ceilings[name] = round(figure)
            lowered.append(name)
            mark = "  lowered"
        noisy = f"  (runs spread {spread:.1%})" if spread > tolerance / 2 else ""
        print(f"  {name:44s} {figure / 1e6:11.2f} M  ceiling {ceiling / 1e6:11.2f} M  {change:+7.2%}{mark}{noisy}")
    if lowered or args.reset or any(name not in book["rows"] for name in got):
        book["rows"] = dict(sorted(ceilings.items()))
        CEILINGS.write_text(json.dumps(book, indent=2) + "\n")
        print(f"wrote {CEILINGS.relative_to(ROOT)}")
    if over:
        print(f"{len(over)} row(s) over their ceiling by more than {tolerance:.1%}: {', '.join(over)}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
