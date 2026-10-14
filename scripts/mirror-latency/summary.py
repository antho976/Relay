#!/usr/bin/env python3
"""One line per run, the medians that matter, for A/B-ing settings or builds.

    summary.py RUN_DIR [RUN_DIR ...]

A run directory holds that run's `engine.log`, `native.log` and `driver.log` (README.md); a
`cpu.txt` beside them, if any, is shown as is.
"""
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
COLUMNS = [
    ("dev>line", "stages", "device: draw -> client line complete"),
    ("decode", "stages", "decode: unit in -> picture out"),
    ("texture", "stages", "gtk: texture built and set"),
    ("paint", "stages", "gtk: paint (snapshot + upload + render)"),
    ("line>pres", "stages", "HOST: line complete -> presented"),
    ("drw>dec", "stages", "TOTAL: draw -> decoded"),
    ("drw>pres", "stages", "TOTAL: draw -> presented"),
    ("tapA>pres", "animtaps", "tap -> presented"),
    ("tapS>dec", "stilltaps", "tap -> decoded"),
    ("tapS>pres", "stilltaps", "tap -> presented"),
]


def main():
    print(f"{'run':28s} " + " ".join(f"{name:>9s}" for name, _, _ in COLUMNS) + "  cpu")
    for run in sys.argv[1:]:
        logs = [os.path.join(run, name) for name in ("engine.log", "native.log", "driver.log")]
        out = subprocess.run([sys.executable, os.path.join(HERE, "analyze.py"), *logs, "--json"], capture_output=True, text=True)
        try:
            report = json.loads(out.stdout)
        except json.JSONDecodeError:
            print(f"{os.path.basename(run.rstrip('/')):28s} (no report: {out.stderr.strip()[-120:]})")
            continue
        cells = []
        for _, section, key in COLUMNS:
            table = report["stages"] if section == "stages" else report["touch"].get(section, {})
            value = table.get(key)
            cells.append(f"{value['median']:9.2f}" if value else f"{'-':>9s}")
        cpu = ""
        try:
            cpu = open(os.path.join(run, "cpu.txt")).read().strip()
        except OSError:
            pass
        print(f"{os.path.basename(run.rstrip('/')):28s} " + " ".join(cells) + f"  {cpu}")


if __name__ == "__main__":
    main()
